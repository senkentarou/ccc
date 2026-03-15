use nucleo::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo::Matcher;
use nucleo::Utf32Str;

use crate::store::session::Session;

/// Maximum number of characters to search per message (1M chars ≈ 4MB).
const MAX_SEARCH_CHARS: usize = 1_000_000;

/// A search result with score and matching info.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SearchResult {
    pub session_index: usize,
    pub score: u32,
    pub matched_message_index: usize,
}

/// Rank sessions by fuzzy matching all messages against the query.
/// Returns session IDs sorted by highest score (descending).
pub fn rank_sessions(sessions: &[Session], query: &str) -> Vec<SearchResult> {
    if query.is_empty() {
        return Vec::new();
    }

    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );
    let mut matcher = Matcher::default();
    let mut results = Vec::new();
    let mut buf = Vec::new();

    for (session_idx, session) in sessions.iter().enumerate() {
        let mut best_score: u32 = 0;
        let mut best_index: usize = 0;

        for msg in &session.messages {
            buf.clear();
            buf.extend(msg.content.chars().take(MAX_SEARCH_CHARS));
            let haystack = Utf32Str::Unicode(&buf);

            if let Some(score) = pattern.score(haystack, &mut matcher) {
                if score > best_score {
                    best_score = score;
                    best_index = msg.index;
                }
            }
        }

        if best_score > 0 {
            results.push(SearchResult {
                session_index: session_idx,
                score: best_score,
                matched_message_index: best_index,
            });
        }
    }

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results
}

/// Check if a specific text matches the query and return the score.
#[allow(dead_code)]
pub fn match_score(query: &str, text: &str) -> Option<u32> {
    if query.is_empty() {
        return None;
    }

    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );
    let mut matcher = Matcher::default();
    let buf: Vec<char> = text.chars().collect();
    let haystack = Utf32Str::Unicode(&buf);

    pattern.score(haystack, &mut matcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::session::{Message, Role};

    fn make_session(id: &str, messages: Vec<(&str, Role)>) -> Session {
        let msgs: Vec<Message> = messages
            .into_iter()
            .enumerate()
            .map(|(i, (content, role))| Message {
                session_id: id.to_string(),
                index: i,
                role,
                content: content.to_string(),
                timestamp: None,
            })
            .collect();

        Session {
            session_id: id.to_string(),
            project_path: String::new(),
            first_timestamp: None,
            last_timestamp: None,
            message_count: msgs.len(),
            cwd: String::new(),
            messages: msgs,
            git_branch: None,
        }
    }

    #[test]
    fn test_rank_sessions_basic() {
        let sessions = vec![
            make_session(
                "s1",
                vec![
                    ("How to build an HTTP server in Rust?", Role::User),
                    ("Use axum framework", Role::Assistant),
                ],
            ),
            make_session(
                "s2",
                vec![
                    ("Python data analysis", Role::User),
                    ("Use pandas", Role::Assistant),
                ],
            ),
        ];

        let results = rank_sessions(&sessions, "http server rust");
        assert!(!results.is_empty());
        assert_eq!(results[0].session_index, 0);
    }

    #[test]
    fn test_rank_sessions_empty_query() {
        let sessions = vec![make_session("s1", vec![("Hello", Role::User)])];

        let results = rank_sessions(&sessions, "");
        assert!(results.is_empty());
    }

    #[test]
    fn test_rank_sessions_no_match() {
        let sessions = vec![make_session("s1", vec![("Hello world", Role::User)])];

        let results = rank_sessions(&sessions, "zzzzxxxxxyyyyyyy");
        assert!(results.is_empty());
    }

    #[test]
    fn test_match_score_basic() {
        let result = match_score("helo", "Hello world");
        assert!(result.is_some());
    }

    #[test]
    fn test_match_score_empty_query() {
        let result = match_score("", "Hello");
        assert!(result.is_none());
    }

    #[test]
    fn test_fuzzy_matching() {
        let sessions = vec![
            make_session("s1", vec![("RustでHTTPサーバーを作りたい", Role::User)]),
            make_session("s2", vec![("Pythonでデータ分析", Role::User)]),
        ];

        let results = rank_sessions(&sessions, "HTTP");
        assert!(!results.is_empty());
        assert_eq!(results[0].session_index, 0);
    }
}
