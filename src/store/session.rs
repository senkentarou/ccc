use chrono::{DateTime, Utc};
use rayon::prelude::*;
#[cfg(test)]
use std::path::Path;

use crate::parser::jsonl;

/// Message role enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// Filter for which messages to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MessageFilter {
    User,
    Assistant,
    Both,
}

/// A single message in a session.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Message {
    pub session_id: String,
    pub index: usize,
    pub role: Role,
    pub content: String,
    pub timestamp: Option<DateTime<Utc>>,
}

/// A session containing multiple messages.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Session {
    pub session_id: String,
    pub project_path: String,
    pub first_timestamp: Option<DateTime<Utc>>,
    pub last_timestamp: Option<DateTime<Utc>>,
    pub message_count: usize,
    pub cwd: String,
    pub messages: Vec<Message>,
    pub git_branch: Option<String>,
}

/// In-memory session store.
pub struct SessionStore {
    sessions: Vec<Session>,
}

impl SessionStore {
    /// Load sessions from a project path (parallel).
    pub fn load(project_path: &str) -> anyhow::Result<Self> {
        let files = jsonl::discover_session_files(project_path)?;

        let mut sessions: Vec<Session> = files
            .par_iter()
            .filter_map(|file| match jsonl::parse_session_file(file) {
                Ok(Some(session)) => Some(session),
                Ok(None) => None,
                Err(e) => {
                    eprintln!("Warning: Failed to parse {}: {}", file.display(), e);
                    None
                }
            })
            .collect();

        // Sort by last_timestamp descending (newest first)
        sessions.sort_by(|a, b| b.last_timestamp.cmp(&a.last_timestamp));

        Ok(Self { sessions })
    }

    /// Load sessions from a list of file paths (for testing).
    #[cfg(test)]
    pub fn load_from_files(files: &[&Path]) -> anyhow::Result<Self> {
        let mut sessions = Vec::new();

        for file in files {
            match jsonl::parse_session_file(file) {
                Ok(Some(session)) => sessions.push(session),
                Ok(None) => {}
                Err(e) => {
                    eprintln!("Warning: Failed to parse {}: {}", file.display(), e);
                }
            }
        }

        sessions.sort_by(|a, b| b.last_timestamp.cmp(&a.last_timestamp));

        Ok(Self { sessions })
    }

    /// Get all sessions.
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    /// Get mutable sessions (for reordering by search score).
    #[allow(dead_code)]
    pub fn sessions_mut(&mut self) -> &mut Vec<Session> {
        &mut self.sessions
    }

    /// Get user messages for a session.
    #[allow(dead_code)]
    pub fn user_messages(&self, session_id: &str) -> Vec<&Message> {
        self.sessions
            .iter()
            .find(|s| s.session_id == session_id)
            .map(|s| s.messages.iter().filter(|m| m.role == Role::User).collect())
            .unwrap_or_default()
    }

    /// Get messages filtered by role.
    #[allow(dead_code)]
    pub fn filtered_messages(&self, session_id: &str, filter: MessageFilter) -> Vec<&Message> {
        self.sessions
            .iter()
            .find(|s| s.session_id == session_id)
            .map(|s| {
                s.messages
                    .iter()
                    .filter(|m| match filter {
                        MessageFilter::User => m.role == Role::User,
                        MessageFilter::Assistant => m.role == Role::Assistant,
                        MessageFilter::Both => true,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get unique branch names from all sessions, sorted alphabetically.
    pub fn branches(&self) -> Vec<String> {
        let mut branches: Vec<String> = self
            .sessions
            .iter()
            .filter_map(|s| s.git_branch.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        branches.sort();
        branches
    }

    /// Get session indices filtered by branch.
    /// If `branch` is None, return all sessions. Otherwise filter to matching branch.
    #[allow(dead_code)]
    pub fn sessions_by_branch(&self, branch: Option<&str>) -> Vec<usize> {
        match branch {
            None => (0..self.sessions.len()).collect(),
            Some(b) => self
                .sessions
                .iter()
                .enumerate()
                .filter(|(_, s)| s.git_branch.as_deref() == Some(b))
                .map(|(i, _)| i)
                .collect(),
        }
    }

    /// Get total number of sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Check if store is empty.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use tempfile::NamedTempFile;

    fn create_test_session_file() -> NamedTempFile {
        let jsonl = r#"{"type":"user","message":{"role":"user","content":"Hello"},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"test1","cwd":"/home/user/project"}
{"type":"assistant","message":{"role":"assistant","content":"Hi!"},"timestamp":"2026-03-15T14:30:01.000Z","sessionId":"test1","cwd":"/home/user/project"}
{"type":"user","message":{"role":"user","content":"How are you?"},"timestamp":"2026-03-15T14:30:02.000Z","sessionId":"test1","cwd":"/home/user/project"}"#;

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", jsonl).unwrap();
        file
    }

    #[test]
    fn test_load_from_files() {
        let file = create_test_session_file();
        let store = SessionStore::load_from_files(&[file.path()]).unwrap();

        assert_eq!(store.session_count(), 1);
        assert!(!store.is_empty());
    }

    #[test]
    fn test_user_messages() {
        let file = create_test_session_file();
        let store = SessionStore::load_from_files(&[file.path()]).unwrap();

        let session_id = &store.sessions()[0].session_id;
        let user_msgs = store.user_messages(session_id);

        assert_eq!(user_msgs.len(), 2);
        assert_eq!(user_msgs[0].content, "Hello");
        assert_eq!(user_msgs[1].content, "How are you?");
    }

    #[test]
    fn test_filtered_messages() {
        let file = create_test_session_file();
        let store = SessionStore::load_from_files(&[file.path()]).unwrap();

        let session_id = &store.sessions()[0].session_id;

        let user = store.filtered_messages(session_id, MessageFilter::User);
        assert_eq!(user.len(), 2);

        let assistant = store.filtered_messages(session_id, MessageFilter::Assistant);
        assert_eq!(assistant.len(), 1);

        let both = store.filtered_messages(session_id, MessageFilter::Both);
        assert_eq!(both.len(), 3);
    }

    fn create_session_with_branch(branch: Option<&str>) -> NamedTempFile {
        let branch_field = match branch {
            Some(b) => format!(r#","gitBranch":"{}""#, b),
            None => String::new(),
        };
        let jsonl = format!(
            r#"{{"type":"user","message":{{"role":"user","content":"Hello"}},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"test1","cwd":"/home/user/project"{}}}"#,
            branch_field
        );
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", jsonl).unwrap();
        file
    }

    #[test]
    fn test_branches_unique_sorted() {
        let f1 = create_session_with_branch(Some("main"));
        let f2 = create_session_with_branch(Some("feat/api"));
        let f3 = create_session_with_branch(Some("main"));
        let f4 = create_session_with_branch(None);
        let store =
            SessionStore::load_from_files(&[f1.path(), f2.path(), f3.path(), f4.path()]).unwrap();

        let branches = store.branches();
        assert_eq!(branches, vec!["feat/api", "main"]);
    }

    #[test]
    fn test_sessions_by_branch_all() {
        let f1 = create_session_with_branch(Some("main"));
        let f2 = create_session_with_branch(Some("feat/api"));
        let store = SessionStore::load_from_files(&[f1.path(), f2.path()]).unwrap();

        let indices = store.sessions_by_branch(None);
        assert_eq!(indices.len(), 2);
    }

    #[test]
    fn test_sessions_by_branch_filtered() {
        let f1 = create_session_with_branch(Some("main"));
        let f2 = create_session_with_branch(Some("feat/api"));
        let f3 = create_session_with_branch(Some("main"));
        let store = SessionStore::load_from_files(&[f1.path(), f2.path(), f3.path()]).unwrap();

        let main_sessions = store.sessions_by_branch(Some("main"));
        assert_eq!(main_sessions.len(), 2);
        for idx in &main_sessions {
            assert_eq!(store.sessions()[*idx].git_branch.as_deref(), Some("main"));
        }

        let api_sessions = store.sessions_by_branch(Some("feat/api"));
        assert_eq!(api_sessions.len(), 1);
    }

    #[test]
    fn test_nonexistent_session() {
        let file = create_test_session_file();
        let store = SessionStore::load_from_files(&[file.path()]).unwrap();

        let msgs = store.user_messages("nonexistent");
        assert!(msgs.is_empty());
    }
}
