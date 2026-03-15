use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Layout areas for the TUI.
///
/// ```text
/// 40% left                     60% right
/// ┌───────────────────────┬────────────────────────────┐
/// │ search_area            │                            │
/// ├───────────────────────┤  preview_pane               │
/// │ message_area           │                            │
/// ├───────────────────────┤                            │
/// │ branch_area            │                            │
/// └───────────────────────┴────────────────────────────┘
/// status_bar (1 line)
/// ```
pub struct AppLayout {
    /// Entire left pane area (for drawing outer border).
    pub left_pane: Rect,
    /// Entire right pane area (for drawing outer border).
    pub right_pane: Rect,
    /// Search input area (inside left pane border).
    pub search_area: Rect,
    /// Message list area (inside left pane border).
    pub message_area: Rect,
    /// Branch list area (inside left pane border).
    pub branch_area: Rect,
    /// Preview content area (inside right pane border).
    pub preview_area: Rect,
    /// Status bar at the bottom.
    pub status_bar: Rect,
}

/// Calculate layout. `branch_count` determines the height of the branch pane.
pub fn calculate_layout(area: Rect, branch_count: usize) -> AppLayout {
    // Main split: content area + status bar
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // content
            Constraint::Length(1), // status bar
        ])
        .split(area);

    let content_area = vertical[0];
    let status_bar = vertical[1];

    // Horizontal split: left 40% / right 60%
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(content_area);

    let left_pane = horizontal[0];
    let right_pane = horizontal[1];

    // Inner area of left pane (accounting for borders: top, left, bottom — no right border)
    let left_inner = Rect {
        x: left_pane.x + 1,
        y: left_pane.y + 1,
        width: left_pane.width.saturating_sub(1), // only left border, right is shared
        height: left_pane.height.saturating_sub(2), // top + bottom borders
    };

    // Inner area of right pane (accounting for borders: all sides)
    let preview_area = Rect {
        x: right_pane.x + 1,
        y: right_pane.y + 1,
        width: right_pane.width.saturating_sub(2),
        height: right_pane.height.saturating_sub(2),
    };

    // Branch area height: 1 line per branch, capped at 8 lines, minimum 1
    let branch_height = (branch_count as u16).clamp(1, 8);

    // Split left inner: search (1 line) + messages (flex) + branches (dynamic)
    // We use 1 extra line for each horizontal separator
    let left_vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),             // search input (1 line)
            Constraint::Length(1),             // separator line
            Constraint::Min(1),                // messages (flex)
            Constraint::Length(1),             // separator line
            Constraint::Length(branch_height), // branches
        ])
        .split(left_inner);

    AppLayout {
        left_pane,
        right_pane,
        search_area: left_vertical[0],
        message_area: left_vertical[2],
        branch_area: left_vertical[4],
        preview_area,
        status_bar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_calculation() {
        let area = Rect::new(0, 0, 120, 40);
        let layout = calculate_layout(area, 4);

        // Status bar should be 1 line at the bottom
        assert_eq!(layout.status_bar.height, 1);

        // Left pane starts at x=0
        assert_eq!(layout.left_pane.x, 0);

        // Right pane is to the right
        assert!(layout.right_pane.x > 0);

        // Search area inside left pane
        assert!(layout.search_area.width > 0);
        assert_eq!(layout.search_area.height, 1);

        // Message area below search
        assert!(layout.message_area.y > layout.search_area.y);
        assert!(layout.message_area.height > 0);

        // Branch area below messages
        assert!(layout.branch_area.y > layout.message_area.y);
        assert_eq!(layout.branch_area.height, 4);

        // Preview area inside right pane
        assert!(layout.preview_area.width > 0);
        assert!(layout.preview_area.height > 0);
    }

    #[test]
    fn test_minimum_size() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = calculate_layout(area, 3);

        assert!(layout.search_area.width > 0);
        assert!(layout.message_area.width > 0);
        assert!(layout.branch_area.width > 0);
        assert!(layout.preview_area.width > 0);
    }

    #[test]
    fn test_branch_height_capped() {
        let area = Rect::new(0, 0, 120, 40);
        let layout = calculate_layout(area, 20);
        assert_eq!(layout.branch_area.height, 8);
    }

    #[test]
    fn test_branch_height_minimum() {
        let area = Rect::new(0, 0, 120, 40);
        let layout = calculate_layout(area, 0);
        assert_eq!(layout.branch_area.height, 1);
    }
}
