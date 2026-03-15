use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::store::session::{Message, Role, Session};

/// Raw JSONL record from Claude Code session files.
#[derive(Debug, Deserialize)]
struct RawRecord {
    #[serde(rename = "type")]
    record_type: Option<String>,
    message: Option<RawMessage>,
    timestamp: Option<String>,
    #[serde(rename = "sessionId")]
    #[allow(dead_code)]
    session_id: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "gitBranch")]
    git_branch: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    role: Option<String>,
    content: Option<ContentValue>,
}

/// Content can be a plain string or an array of content blocks.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ContentValue {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: Option<String>,
    text: Option<String>,
}

impl ContentValue {
    fn extract_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| {
                    if b.block_type.as_deref() == Some("text") {
                        b.text.clone()
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// Clean message content by parsing special tags.
///
/// - `<command-message>X</command-message>\n<command-name>/Y</command-name>` → `/Y`
/// - `<command-args>...</command-args>` → appended to command name
/// - `<local-command-caveat>...</local-command-caveat>` → returns None (skip)
/// - `[Request interrupted by user]` etc. → kept as-is
fn clean_content(content: &str) -> Option<String> {
    // Skip meta/system messages
    if content.contains("<local-command-caveat>") || content.contains("<local-command-stdout>") {
        return None;
    }

    // Skip interruption markers (zero information value)
    let trimmed = content.trim();
    if trimmed == "[Request interrupted by user]"
        || trimmed == "[Request interrupted by user for tool use]"
    {
        return None;
    }

    // Parse command messages: extract command name and optional args
    if content.contains("<command-message>") {
        let cmd_name = extract_tag(content, "command-name");
        let cmd_args = extract_tag(content, "command-args");

        if let Some(name) = cmd_name {
            return if let Some(args) = cmd_args {
                Some(format!("{name} {args}"))
            } else {
                Some(name)
            };
        }
    }

    Some(content.to_string())
}

/// Extract content between `<tag>` and `</tag>`.
fn extract_tag(content: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = content.find(&open)?;
    let end = content.find(&close)?;
    let inner_start = start + open.len();
    if inner_start <= end {
        Some(content[inner_start..end].to_string())
    } else {
        None
    }
}

/// Result of parsing a session file, including parse statistics.
pub struct ParseResult {
    pub session: Option<Session>,
    /// Number of lines that could not be parsed as JSON.
    #[allow(dead_code)]
    pub skipped_lines: usize,
}

/// Parse a single JSONL session file into a `Session` with its `Message`s.
#[allow(clippy::too_many_lines)]
pub fn parse_session_file(path: &Path) -> Result<ParseResult> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read session file: {}", path.display()))?;

    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut messages = Vec::new();
    let mut cwd = String::new();
    let mut git_branch: Option<String> = None;
    let mut index = 0usize;
    let mut skipped_lines = 0usize;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Ok(record) = serde_json::from_str::<RawRecord>(line) else {
            skipped_lines += 1;
            continue;
        };

        // Extract cwd from any record that has it
        if cwd.is_empty() {
            if let Some(ref c) = record.cwd {
                cwd.clone_from(c);
            }
        }

        // Extract gitBranch from the first record that has it
        if git_branch.is_none() {
            if let Some(ref branch) = record.git_branch {
                git_branch = Some(branch.clone());
            }
        }

        let record_type = match record.record_type.as_deref() {
            Some("user" | "assistant") => record.record_type.as_deref().unwrap(),
            _ => continue,
        };

        let Some(raw_msg) = record.message else {
            continue;
        };

        let role = match raw_msg.role.as_deref() {
            Some("user") => Role::User,
            Some("assistant") => Role::Assistant,
            _ => continue,
        };

        // Validate record_type matches role
        let expected_type = match role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        if record_type != expected_type {
            continue;
        }

        let raw_text = match raw_msg.content {
            Some(cv) => cv.extract_text(),
            None => continue,
        };

        if raw_text.is_empty() {
            continue;
        }

        // Clean special tags; None means skip this message
        let content_text = match clean_content(&raw_text) {
            Some(t) if !t.is_empty() => t,
            _ => continue,
        };

        let timestamp = record
            .timestamp
            .as_deref()
            .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
            .map(|dt| dt.with_timezone(&Utc));

        messages.push(Message {
            session_id: session_id.clone(),
            index,
            role,
            content: content_text,
            timestamp,
        });

        index += 1;
    }

    if messages.is_empty() {
        return Ok(ParseResult {
            session: None,
            skipped_lines,
        });
    }

    let first_timestamp = messages.iter().filter_map(|m| m.timestamp).min();
    let last_timestamp = messages.iter().filter_map(|m| m.timestamp).max();
    let message_count = messages.len();

    let session = Session {
        session_id,
        project_path: path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        first_timestamp,
        last_timestamp,
        message_count,
        cwd,
        messages,
        git_branch,
    };

    Ok(ParseResult {
        session: Some(session),
        skipped_lines,
    })
}

/// Resolve a project path to its hash used by Claude Code.
/// Claude Code uses a hash of the absolute path as the directory name.
pub fn resolve_project_hash(project_path: &str) -> String {
    // Claude Code replaces all `/` with `-`, keeping the leading `-`.
    // e.g. "/Users/user/work/project" → "-Users-user-work-project"
    project_path.replace('/', "-")
}

/// Discover all .jsonl session files for a given project path.
pub fn discover_session_files(project_path: &str) -> Result<Vec<PathBuf>> {
    let claude_dir = dirs::home_dir()
        .context("Could not determine home directory")?
        .join(".claude")
        .join("projects");

    let hash = resolve_project_hash(project_path);
    let project_dir = claude_dir.join(&hash);

    if !project_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(&project_dir)
        .with_context(|| format!("Failed to read directory: {}", project_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }

    // Sort by modification time, newest first
    files.sort_by(|a, b| {
        let time_a = fs::metadata(a).and_then(|m| m.modified()).ok();
        let time_b = fs::metadata(b).and_then(|m| m.modified()).ok();
        time_b.cmp(&time_a)
    });

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_jsonl(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        file
    }

    #[test]
    fn test_parse_simple_session() {
        let jsonl = r#"{"type":"user","message":{"role":"user","content":"Hello"},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123","cwd":"/home/user/project"}
{"type":"assistant","message":{"role":"assistant","content":"Hi there!"},"timestamp":"2026-03-15T14:30:01.000Z","sessionId":"abc123","cwd":"/home/user/project"}"#;

        let file = create_temp_jsonl(jsonl);
        let session = parse_session_file(file.path()).unwrap().session.unwrap();

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, Role::User);
        assert_eq!(session.messages[0].content, "Hello");
        assert_eq!(session.messages[1].role, Role::Assistant);
        assert_eq!(session.messages[1].content, "Hi there!");
        assert_eq!(session.cwd, "/home/user/project");
    }

    #[test]
    fn test_parse_content_blocks() {
        let jsonl = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Part 1"},{"type":"text","text":"Part 2"},{"type":"tool_use","id":"123"}]},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123"}"#;

        let file = create_temp_jsonl(jsonl);
        let session = parse_session_file(file.path()).unwrap().session.unwrap();

        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].content, "Part 1\nPart 2");
    }

    #[test]
    fn test_skip_malformed_lines() {
        let jsonl = r#"not valid json
{"type":"user","message":{"role":"user","content":"Hello"},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123"}
{"type":"result","subtype":"success"}
"#;

        let file = create_temp_jsonl(jsonl);
        let result = parse_session_file(file.path()).unwrap();
        let session = result.session.unwrap();

        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].content, "Hello");
        assert_eq!(result.skipped_lines, 1); // "not valid json" was skipped
    }

    #[test]
    fn test_empty_file_returns_none() {
        let file = create_temp_jsonl("");
        let result = parse_session_file(file.path()).unwrap();
        assert!(result.session.is_none());
    }

    #[test]
    fn test_no_user_assistant_returns_none() {
        let jsonl =
            r#"{"type":"result","subtype":"success","timestamp":"2026-03-15T14:30:00.000Z"}"#;
        let file = create_temp_jsonl(jsonl);
        let result = parse_session_file(file.path()).unwrap();
        assert!(result.session.is_none());
    }

    #[test]
    fn test_resolve_project_hash() {
        let hash = resolve_project_hash("/Users/user/work/my-project");
        assert_eq!(hash, "-Users-user-work-my-project");
    }

    #[test]
    fn test_parse_empty_content_skipped() {
        let jsonl = r#"{"type":"user","message":{"role":"user","content":""},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123"}"#;
        let file = create_temp_jsonl(jsonl);
        let result = parse_session_file(file.path()).unwrap();
        assert!(result.session.is_none());
    }

    #[test]
    fn test_parse_git_branch() {
        let jsonl = r#"{"type":"user","message":{"role":"user","content":"Hello"},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123","cwd":"/home/user/project","gitBranch":"feat/api"}"#;

        let file = create_temp_jsonl(jsonl);
        let session = parse_session_file(file.path()).unwrap().session.unwrap();

        assert_eq!(session.git_branch, Some("feat/api".to_string()));
    }

    #[test]
    fn test_parse_no_git_branch() {
        let jsonl = r#"{"type":"user","message":{"role":"user","content":"Hello"},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123","cwd":"/home/user/project"}"#;

        let file = create_temp_jsonl(jsonl);
        let session = parse_session_file(file.path()).unwrap().session.unwrap();

        assert_eq!(session.git_branch, None);
    }

    #[test]
    fn test_parse_git_branch_uses_first_value() {
        let jsonl = r#"{"type":"user","message":{"role":"user","content":"Hello"},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123","gitBranch":"main"}
{"type":"assistant","message":{"role":"assistant","content":"Hi!"},"timestamp":"2026-03-15T14:30:01.000Z","sessionId":"abc123","gitBranch":"feat/new"}"#;

        let file = create_temp_jsonl(jsonl);
        let session = parse_session_file(file.path()).unwrap().session.unwrap();

        assert_eq!(session.git_branch, Some("main".to_string()));
    }

    #[test]
    fn test_parse_command_message() {
        let jsonl = r#"{"type":"user","message":{"role":"user","content":"<command-message>init-project</command-message>\n<command-name>/init-project</command-name>"},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123"}
{"type":"assistant","message":{"role":"assistant","content":"Setting up..."},"timestamp":"2026-03-15T14:30:01.000Z","sessionId":"abc123"}"#;

        let file = create_temp_jsonl(jsonl);
        let session = parse_session_file(file.path()).unwrap().session.unwrap();

        assert_eq!(session.messages[0].content, "/init-project");
    }

    #[test]
    fn test_parse_command_message_with_args() {
        let jsonl = r#"{"type":"user","message":{"role":"user","content":"<command-message>create-pr</command-message>\n<command-name>/create-pr</command-name>\n<command-args>Fix login bug</command-args>"},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123"}
{"type":"assistant","message":{"role":"assistant","content":"Creating PR..."},"timestamp":"2026-03-15T14:30:01.000Z","sessionId":"abc123"}"#;

        let file = create_temp_jsonl(jsonl);
        let session = parse_session_file(file.path()).unwrap().session.unwrap();

        assert_eq!(session.messages[0].content, "/create-pr Fix login bug");
    }

    #[test]
    fn test_parse_local_command_caveat_skipped() {
        let jsonl = r#"{"type":"user","message":{"role":"user","content":"<local-command-caveat>Caveat: The messages below were generated by the user while running local commands.</local-command-caveat>"},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123"}
{"type":"assistant","message":{"role":"assistant","content":"Hi!"},"timestamp":"2026-03-15T14:30:01.000Z","sessionId":"abc123"}"#;

        let file = create_temp_jsonl(jsonl);
        let session = parse_session_file(file.path()).unwrap().session.unwrap();

        // local-command-caveat message should be skipped
        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].role, Role::Assistant);
    }

    #[test]
    fn test_parse_interrupted_message_skipped() {
        let jsonl = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"[Request interrupted by user]"}]},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123"}
{"type":"assistant","message":{"role":"assistant","content":"Ok"},"timestamp":"2026-03-15T14:30:01.000Z","sessionId":"abc123"}"#;

        let file = create_temp_jsonl(jsonl);
        let session = parse_session_file(file.path()).unwrap().session.unwrap();

        // Interruption messages are skipped
        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].role, Role::Assistant);
    }

    #[test]
    fn test_parse_interrupted_for_tool_use_skipped() {
        let jsonl = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"[Request interrupted by user for tool use]"}]},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123"}
{"type":"assistant","message":{"role":"assistant","content":"Ok"},"timestamp":"2026-03-15T14:30:01.000Z","sessionId":"abc123"}"#;

        let file = create_temp_jsonl(jsonl);
        let session = parse_session_file(file.path()).unwrap().session.unwrap();

        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].role, Role::Assistant);
    }

    #[test]
    fn test_message_ordering() {
        let jsonl = r#"{"type":"user","message":{"role":"user","content":"First"},"timestamp":"2026-03-15T14:30:00.000Z","sessionId":"abc123"}
{"type":"assistant","message":{"role":"assistant","content":"Second"},"timestamp":"2026-03-15T14:30:01.000Z","sessionId":"abc123"}
{"type":"user","message":{"role":"user","content":"Third"},"timestamp":"2026-03-15T14:30:02.000Z","sessionId":"abc123"}"#;

        let file = create_temp_jsonl(jsonl);
        let session = parse_session_file(file.path()).unwrap().session.unwrap();

        assert_eq!(session.messages.len(), 3);
        assert_eq!(session.messages[0].index, 0);
        assert_eq!(session.messages[1].index, 1);
        assert_eq!(session.messages[2].index, 2);
    }
}
