use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};
use ratatui::Frame;

/// Render the branch list pane (bottom of left pane).
pub fn render_branch_pane(
    frame: &mut Frame,
    area: Rect,
    branches: &[String],
    selected: usize,
    current_branch: Option<&str>,
) {
    let items: Vec<ListItem> = branches
        .iter()
        .enumerate()
        .map(|(i, branch)| {
            let is_current = current_branch.is_some_and(|cb| cb == branch);

            let style = if is_current {
                Style::default().fg(Color::Reset)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let prefix = if i == selected { "▶ " } else { "  " };

            let prefix_style = if i == selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![
                Span::styled(prefix, prefix_style),
                Span::styled(branch.clone(), style),
            ]))
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if !branches.is_empty() {
        state.select(Some(selected));
    }

    frame.render_stateful_widget(list, area, &mut state);
}
