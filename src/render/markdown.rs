use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Maximum input size for markdown rendering (1MB) to prevent `DoS`.
const MAX_INPUT_SIZE: usize = 1_048_576;

/// Convert markdown text to ratatui Lines with basic formatting.
#[allow(clippy::too_many_lines)]
pub fn render_markdown(input: &str) -> Vec<Line<'static>> {
    // Enforce size limit (respect UTF-8 char boundary)
    let input = if input.len() > MAX_INPUT_SIZE {
        let mut end = MAX_INPUT_SIZE;
        while end > 0 && !input.is_char_boundary(end) {
            end -= 1;
        }
        &input[..end]
    } else {
        input
    };

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(input, options);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut in_code_block = false;
    let mut list_depth: usize = 0;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    let prefix = "#".repeat(level as usize);
                    let style = Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD);
                    current_spans.push(Span::styled(format!("{prefix} "), style));
                    style_stack.push(style);
                }
                Tag::CodeBlock(_) => {
                    in_code_block = true;
                    // Flush current line
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    lines.push(Line::from(Span::styled(
                        "───────────────────────────────",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                Tag::List(_) => {
                    list_depth += 1;
                }
                Tag::Item => {
                    let indent = "  ".repeat(list_depth.saturating_sub(1));
                    current_spans.push(Span::styled(
                        format!("{indent}• "),
                        Style::default().fg(Color::Yellow),
                    ));
                }
                Tag::Emphasis => {
                    let style = Style::default().add_modifier(Modifier::ITALIC);
                    style_stack.push(style);
                }
                Tag::Strong => {
                    let style = Style::default().add_modifier(Modifier::BOLD);
                    style_stack.push(style);
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    style_stack.pop();
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    lines.push(Line::from(Span::styled(
                        "───────────────────────────────",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                TagEnd::List(_) => {
                    list_depth = list_depth.saturating_sub(1);
                }
                TagEnd::Item => {
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                }
                TagEnd::Paragraph => {
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                    lines.push(Line::from(""));
                }
                TagEnd::Emphasis | TagEnd::Strong => {
                    style_stack.pop();
                }
                _ => {}
            },
            Event::Text(text) => {
                if in_code_block {
                    // Render code block lines individually
                    for code_line in text.to_string().lines() {
                        current_spans.push(Span::styled(
                            format!("  {code_line}"),
                            Style::default().fg(Color::Green),
                        ));
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                } else {
                    let style = style_stack.last().copied().unwrap_or_default();
                    current_spans.push(Span::styled(text.to_string(), style));
                }
            }
            Event::Code(code) => {
                current_spans.push(Span::styled(
                    format!("`{code}`"),
                    Style::default().fg(Color::Green),
                ));
            }
            Event::SoftBreak | Event::HardBreak => {
                lines.push(Line::from(std::mem::take(&mut current_spans)));
            }
            _ => {}
        }
    }

    // Flush remaining spans
    if !current_spans.is_empty() {
        lines.push(Line::from(current_spans));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        let lines = render_markdown("Hello world");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_heading() {
        let lines = render_markdown("# Title\n\nSome text");
        assert!(!lines.is_empty());
        // First line should contain the heading
        let first_line_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(first_line_text.contains("# Title"));
    }

    #[test]
    fn test_code_block() {
        let input = "```rust\nfn main() {}\n```";
        let lines = render_markdown(input);
        assert!(lines.len() >= 3); // separator + code + separator
    }

    #[test]
    fn test_inline_code() {
        let lines = render_markdown("Use `cargo build` to compile");
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref().to_string()))
            .collect();
        assert!(all_text.contains("`cargo build`"));
    }

    #[test]
    fn test_list() {
        let input = "- Item 1\n- Item 2\n- Item 3";
        let lines = render_markdown(input);
        assert!(lines.len() >= 3);
    }

    #[test]
    fn test_size_limit() {
        let large_input = "x".repeat(MAX_INPUT_SIZE + 1000);
        // Should not panic
        let _lines = render_markdown(&large_input);
    }

    #[test]
    fn test_empty_input() {
        let lines = render_markdown("");
        assert!(lines.is_empty());
    }

    #[test]
    fn test_bold_and_italic() {
        let lines = render_markdown("**bold** and *italic*");
        assert!(!lines.is_empty());
    }
}
