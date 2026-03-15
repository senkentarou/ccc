use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::PreviewMode;
use crate::render::markdown;
use crate::store::session::{Message, Role};
use crate::tui::message_pane::truncate_str;

/// Render the preview pane (right side).
#[allow(clippy::cast_possible_truncation)] // ratatui uses u16 for coordinates
pub fn render_preview_pane(
    frame: &mut Frame,
    area: Rect,
    messages: &[&Message],
    mode: PreviewMode,
    scroll: u16,
) {
    if messages.is_empty() {
        let paragraph = Paragraph::new("No messages to display")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let lines = match mode {
        PreviewMode::Full => render_full(messages),
        PreviewMode::Short => render_short(messages, area.width.saturating_sub(8) as usize),
    };

    // Cap scroll so we don't scroll past content
    let max_scroll = (lines.len() as u16).saturating_sub(area.height);
    let capped_scroll = scroll.min(max_scroll);

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((capped_scroll, 0));

    frame.render_widget(paragraph, area);
}

/// Full preview: markdown-rendered messages with role headers.
fn render_full(messages: &[&Message]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for (i, msg) in messages.iter().enumerate() {
        if i > 0 && msg.role == Role::User {
            // Add separator between user turns
            lines.push(Line::from(Span::styled(
                "────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
        }

        let (label, label_style) = match msg.role {
            Role::User => (
                "User:",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Role::Assistant => (
                "Assistant:",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        };

        lines.push(Line::from(Span::styled(label.to_string(), label_style)));

        let rendered = markdown::render_markdown(&msg.content);
        lines.extend(rendered);

        lines.push(Line::from(""));
    }

    lines
}

/// Short preview: one-line pairs of User/Assistant.
fn render_short(messages: &[&Message], max_content_width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut i = 0;

    while i < messages.len() {
        let msg = messages[i];

        if msg.role == Role::User {
            // Check if we need a separator (not the first user message)
            if !lines.is_empty() {
                lines.push(Line::from(Span::styled(
                    "────────────────────────────────",
                    Style::default().fg(Color::DarkGray),
                )));
            }

            // User line
            let content = truncate_str(&msg.content, max_content_width);
            lines.push(Line::from(vec![
                Span::styled(
                    "User: ",
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(content),
            ]));

            // Look for next assistant message
            if i + 1 < messages.len() && messages[i + 1].role == Role::Assistant {
                let asst = messages[i + 1];
                let asst_content = truncate_str(&asst.content, max_content_width);
                lines.push(Line::from(vec![
                    Span::styled(
                        "Asst: ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(asst_content),
                ]));
                i += 2;
                continue;
            }
        }

        i += 1;
    }

    lines
}
