use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::MessageListItem;

/// Render the message list pane showing cross-session user messages with separators.
pub fn render_message_pane(
    frame: &mut Frame,
    area: Rect,
    items: &[MessageListItem],
    selected: usize,
    current_branch: Option<&str>,
) {
    if items.is_empty() {
        let paragraph = Paragraph::new("No matches")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let max_width = area.width.saturating_sub(4) as usize;

    let list_items: Vec<ListItem> = items
        .iter()
        .map(|item| match item {
            MessageListItem::Separator { session_id, branch } => {
                let short_id = if session_id.len() > 8 {
                    &session_id[..8]
                } else {
                    session_id
                };

                // Build label: "─ <id> <branch> ─"
                let mut label = format!("─ {short_id}");
                if let Some(branch_name) = branch {
                    label.push(' ');
                    label.push_str(branch_name);
                }
                label.push(' ');

                let label_width = UnicodeWidthStr::width(label.as_str());
                let total_width = area.width as usize;
                let trail_len = total_width.saturating_sub(label_width);
                let trail = "─".repeat(trail_len);

                // Build spans with branch color
                let dim = Style::default().fg(Color::DarkGray);
                let mut spans = vec![Span::styled("─ ", dim)];

                spans.push(Span::styled(short_id.to_string(), dim));

                if let Some(branch_name) = branch {
                    let is_current = current_branch.is_some_and(|cb| cb == branch_name.as_str());
                    let branch_style = if is_current {
                        Style::default().fg(Color::Reset)
                    } else {
                        dim
                    };
                    spans.push(Span::styled(" ", dim));
                    spans.push(Span::styled(branch_name.clone(), branch_style));
                }

                spans.push(Span::styled(format!(" {trail}"), dim));

                ListItem::new(Line::from(spans))
            }
            MessageListItem::UserMessage {
                content_first_line, ..
            } => {
                let truncated = truncate_str(content_first_line, max_width);
                let style = if content_first_line.starts_with('/') {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(truncated, style)))
            }
        })
        .collect();

    let list = List::new(list_items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected));
    }

    frame.render_stateful_widget(list, area, &mut state);
}

/// Truncate a string to fit within `max_width`, considering unicode width.
pub fn truncate_str(s: &str, max_width: usize) -> String {
    let first_line = s.lines().next().unwrap_or("");

    if UnicodeWidthStr::width(first_line) <= max_width {
        return first_line.to_string();
    }

    let mut result = String::new();
    let mut width = 0;

    for ch in first_line.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width + 1 > max_width {
            result.push('…');
            break;
        }
        result.push(ch);
        width += ch_width;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate_str("Hello", 10), "Hello");
    }

    #[test]
    fn test_truncate_long_string() {
        let result = truncate_str("This is a very long string", 10);
        assert!(UnicodeWidthStr::width(result.as_str()) <= 10);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn test_truncate_multiline() {
        let result = truncate_str("Line 1\nLine 2\nLine 3", 50);
        assert_eq!(result, "Line 1");
    }

    #[test]
    fn test_truncate_japanese() {
        let result = truncate_str("日本語のテキストです", 10);
        assert!(UnicodeWidthStr::width(result.as_str()) <= 10);
    }
}
