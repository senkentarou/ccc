use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

/// Render the search area (top of left pane).
///
/// Style: ` > query█rest  12/34`
/// - ` > ` prefix
/// - Block cursor via background color highlight
/// - Match count right-aligned
/// - `cursor_pos` is character-based (not byte-based)
#[allow(clippy::cast_possible_truncation)] // ratatui uses u16 for coordinates
pub fn render_search_area(
    frame: &mut Frame,
    area: Rect,
    query: &str,
    cursor_pos: usize,
    match_count: usize,
    total_count: usize,
) {
    let count_text = format!("{match_count}/{total_count}");
    let prefix = "> ";

    if query.is_empty() {
        // Show block cursor at start position + placeholder
        let mut spans = vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::styled(" ", Style::default().fg(Color::Reset).bg(Color::DarkGray)),
            Span::styled("type to search...", Style::default().fg(Color::DarkGray)),
        ];

        // Right-align count
        let content_width: usize = spans
            .iter()
            .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
            .sum();
        let padding = area
            .width
            .saturating_sub(content_width as u16 + count_text.len() as u16);
        if padding > 0 {
            spans.push(Span::raw(" ".repeat(padding as usize)));
        }
        spans.push(Span::styled(
            count_text,
            Style::default().fg(Color::DarkGray),
        ));

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);

        // Set terminal cursor at search input position for IME
        let cursor_x = area.x + prefix.len() as u16;
        frame.set_cursor_position(Position::new(cursor_x, area.y));
        return;
    }

    // Split query at cursor position (character-based)
    let char_count = query.chars().count();
    let byte_pos = query
        .char_indices()
        .nth(cursor_pos)
        .map_or(query.len(), |(i, _)| i);

    let before = &query[..byte_pos];
    let (cursor_char, after) = if cursor_pos < char_count {
        let next_byte = query
            .char_indices()
            .nth(cursor_pos + 1)
            .map_or(query.len(), |(i, _)| i);
        (&query[byte_pos..next_byte], &query[next_byte..])
    } else {
        ("", "")
    };

    let mut spans = vec![
        Span::styled(prefix, Style::default().fg(Color::Cyan)),
        Span::raw(before.to_string()),
    ];

    // Block cursor
    if cursor_char.is_empty() {
        // Cursor at end — show block space
        spans.push(Span::styled(
            " ",
            Style::default().fg(Color::Reset).bg(Color::DarkGray),
        ));
    } else {
        spans.push(Span::styled(
            cursor_char.to_string(),
            Style::default().fg(Color::Reset).bg(Color::DarkGray),
        ));
    }

    if !after.is_empty() {
        spans.push(Span::raw(after.to_string()));
    }

    // Right-align count
    let content_width: usize = spans
        .iter()
        .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
        .sum();
    let padding = area
        .width
        .saturating_sub(content_width as u16 + count_text.len() as u16);
    if padding > 0 {
        spans.push(Span::raw(" ".repeat(padding as usize)));
    }
    spans.push(Span::styled(
        count_text,
        Style::default().fg(Color::DarkGray),
    ));

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);

    // Set terminal cursor at search input position for IME candidate window
    let before_width = UnicodeWidthStr::width(before) as u16;
    let cursor_x = area.x + prefix.len() as u16 + before_width;
    frame.set_cursor_position(Position::new(cursor_x, area.y));
}
