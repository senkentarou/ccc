use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::Duration;

use crate::search::fuzzy;
use crate::store::session::{Role, SessionStore};
use crate::tui::branch_pane::render_branch_pane;
use crate::tui::keybindings::{map_key_event, Action};
use crate::tui::layout::calculate_layout;
use crate::tui::message_pane::render_message_pane;
use crate::tui::preview_pane::render_preview_pane;
use crate::tui::search_bar::render_search_area;

/// Preview display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    Full,
    Short,
}

/// An item in the cross-session message list.
#[derive(Debug, Clone)]
pub enum MessageListItem {
    /// Session separator showing session_id and branch.
    Separator {
        session_id: String,
        branch: Option<String>,
    },
    /// A user message with its session_id and index within the session.
    UserMessage {
        session_id: String,
        #[allow(dead_code)]
        message_index: usize,
        content_first_line: String,
    },
}

/// Application state.
pub struct App {
    pub store: SessionStore,
    pub message_index: usize,
    pub preview_scroll: u16,
    pub search_query: String,
    pub cursor_pos: usize,
    pub status_message: Option<String>,
    pub should_quit: bool,
    pub resume_session_id: Option<String>,
    /// Session order: indices into store.sessions(), possibly reordered by search.
    pub session_order: Vec<usize>,
    /// Tracks pending 'g' for gg command.
    pending_g: bool,
    /// Current git branch detected at startup.
    pub current_branch: Option<String>,
    /// Index into branch_list (0 = "all").
    pub branch_index: usize,
    /// Branch list: first element is "all", followed by unique branch names.
    pub branch_list: Vec<String>,
    /// Cross-session message list (separators + user messages).
    pub message_list: Vec<MessageListItem>,
    /// Total user message count (before filtering).
    pub total_message_count: usize,
    /// Preview mode: Full (markdown) or Short (one-line pairs).
    pub preview_mode: PreviewMode,
}

impl App {
    pub fn new(store: SessionStore, current_branch: Option<String>) -> Self {
        let session_count = store.session_count();
        let session_order: Vec<usize> = (0..session_count).collect();

        let mut branch_list = vec!["all".to_string()];
        branch_list.extend(store.branches());

        let total_message_count = store
            .sessions()
            .iter()
            .flat_map(|s| &s.messages)
            .filter(|m| m.role == Role::User)
            .count();

        let mut app = Self {
            store,
            message_index: 0,
            preview_scroll: 0,
            search_query: String::new(),
            cursor_pos: 0,
            status_message: None,
            should_quit: false,
            resume_session_id: None,
            session_order,
            pending_g: false,
            current_branch,
            branch_index: 0,
            branch_list,
            message_list: Vec::new(),
            total_message_count,
            preview_mode: PreviewMode::Full,
        };
        app.rebuild_message_list();
        app
    }

    /// Run the application event loop.
    pub fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        let action = map_key_event(key, self.pending_g);
                        self.handle_action(action);
                    }
                }
            }
        }

        Ok(())
    }

    /// Rebuild the cross-session message list based on current session_order and branch filter.
    pub fn rebuild_message_list(&mut self) {
        self.message_list.clear();

        let branch_filter = self.selected_branch_owned();

        for &session_idx in &self.session_order {
            let session = match self.store.sessions().get(session_idx) {
                Some(s) => s,
                None => continue,
            };

            // Apply branch filter
            if let Some(ref branch) = branch_filter {
                if session.git_branch.as_deref() != Some(branch.as_str()) {
                    continue;
                }
            }

            let user_messages: Vec<_> = session
                .messages
                .iter()
                .filter(|m| m.role == Role::User)
                .collect();

            if user_messages.is_empty() {
                continue;
            }

            // Add separator
            self.message_list.push(MessageListItem::Separator {
                session_id: session.session_id.clone(),
                branch: session.git_branch.clone(),
            });

            // Add user messages
            for msg in user_messages {
                let first_line = msg.content.lines().next().unwrap_or("").to_string();
                self.message_list.push(MessageListItem::UserMessage {
                    session_id: session.session_id.clone(),
                    message_index: msg.index,
                    content_first_line: first_line,
                });
            }
        }

        // Reset message_index to first UserMessage
        self.message_index = self.first_message_index();
        self.preview_scroll = 0;
    }

    /// Get the index of the first UserMessage item, or 0.
    fn first_message_index(&self) -> usize {
        self.message_list
            .iter()
            .position(|item| matches!(item, MessageListItem::UserMessage { .. }))
            .unwrap_or(0)
    }

    /// Move message_index to the next UserMessage, skipping separators.
    fn message_move_down(&mut self) {
        let start = self.message_index + 1;
        for i in start..self.message_list.len() {
            if matches!(self.message_list[i], MessageListItem::UserMessage { .. }) {
                self.message_index = i;
                self.preview_scroll = 0;
                return;
            }
        }
    }

    /// Move message_index to the previous UserMessage, skipping separators.
    fn message_move_up(&mut self) {
        if self.message_index == 0 {
            return;
        }
        for i in (0..self.message_index).rev() {
            if matches!(self.message_list[i], MessageListItem::UserMessage { .. }) {
                self.message_index = i;
                self.preview_scroll = 0;
                return;
            }
        }
    }

    fn selected_branch_owned(&self) -> Option<String> {
        if self.branch_index == 0 {
            None
        } else {
            self.branch_list.get(self.branch_index).cloned()
        }
    }

    /// Get the session ID of the currently selected message.
    fn selected_session_id(&self) -> Option<String> {
        match self.message_list.get(self.message_index)? {
            MessageListItem::UserMessage { session_id, .. } => Some(session_id.clone()),
            MessageListItem::Separator { session_id, .. } => Some(session_id.clone()),
        }
    }

    /// Count of user messages currently shown in the message list.
    pub fn visible_message_count(&self) -> usize {
        self.message_list
            .iter()
            .filter(|item| matches!(item, MessageListItem::UserMessage { .. }))
            .count()
    }

    /// Handle an action from a key event.
    fn handle_action(&mut self, action: Action) {
        // Clear status message on any key press
        self.status_message = None;

        // Handle pending 'g' state
        match &action {
            Action::ScrollPreviewTop => {
                self.pending_g = false;
            }
            Action::SearchInput('g') if !self.pending_g => {
                self.pending_g = true;
                return; // Wait for next key
            }
            _ => {
                if self.pending_g {
                    // 'g' was pending but next key wasn't 'g' — insert the 'g' into search
                    self.pending_g = false;
                    let byte_idx = self
                        .search_query
                        .char_indices()
                        .nth(self.cursor_pos)
                        .map_or(self.search_query.len(), |(i, _)| i);
                    self.search_query.insert(byte_idx, 'g');
                    self.cursor_pos += 1;
                    self.update_search();
                    // Then handle the current action normally (if it's not another SearchInput)
                    if matches!(action, Action::SearchInput(_) | Action::SearchBackspace) {
                        // fall through to handle the current action
                    } else {
                        return;
                    }
                }
            }
        }

        match action {
            Action::Quit => self.should_quit = true,
            Action::MessageUp => {
                self.message_move_up();
            }
            Action::MessageDown => {
                self.message_move_down();
            }
            Action::BranchUp => {
                if self.branch_index > 0 {
                    self.branch_index -= 1;
                    self.rebuild_message_list();
                }
            }
            Action::BranchDown => {
                if self.branch_index + 1 < self.branch_list.len() {
                    self.branch_index += 1;
                    self.rebuild_message_list();
                }
            }
            Action::ScrollPreviewUp => {
                self.preview_scroll = self.preview_scroll.saturating_sub(10);
            }
            Action::ScrollPreviewDown => {
                self.preview_scroll = self.preview_scroll.saturating_add(10);
            }
            Action::ScrollPreviewTop => {
                self.preview_scroll = 0;
            }
            Action::ScrollPreviewBottom => {
                self.preview_scroll = u16::MAX;
            }
            Action::TogglePreviewMode => {
                self.preview_mode = match self.preview_mode {
                    PreviewMode::Full => PreviewMode::Short,
                    PreviewMode::Short => PreviewMode::Full,
                };
                self.preview_scroll = 0;
            }
            Action::SearchInput(c) => {
                let byte_idx = self
                    .search_query
                    .char_indices()
                    .nth(self.cursor_pos)
                    .map_or(self.search_query.len(), |(i, _)| i);
                self.search_query.insert(byte_idx, c);
                self.cursor_pos += 1;
                self.update_search();
            }
            Action::SearchBackspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    let start = self
                        .search_query
                        .char_indices()
                        .nth(self.cursor_pos)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    let end = self
                        .search_query
                        .char_indices()
                        .nth(self.cursor_pos + 1)
                        .map_or(self.search_query.len(), |(i, _)| i);
                    self.search_query.replace_range(start..end, "");
                    self.update_search();
                }
            }
            Action::SearchClear => {
                self.search_query.clear();
                self.cursor_pos = 0;
                self.update_search();
            }
            Action::Resume => {
                if let Some(session_id) = self.selected_session_id() {
                    self.resume_session_id = Some(session_id);
                    self.should_quit = true;
                }
            }
            Action::CopySessionId => {
                if let Some(session_id) = self.selected_session_id() {
                    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(&session_id)) {
                        Ok(_) => {
                            self.status_message = Some(format!("Copied: {}", session_id));
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Failed to copy: {}", e));
                        }
                    }
                }
            }
            Action::None => {}
        }
    }

    fn update_search(&mut self) {
        if self.search_query.is_empty() {
            self.session_order = (0..self.store.session_count()).collect();
        } else {
            let results = fuzzy::rank_sessions(self.store.sessions(), &self.search_query);

            if results.is_empty() {
                self.session_order = Vec::new();
            } else {
                self.session_order = results
                    .iter()
                    .filter_map(|r| {
                        self.store
                            .sessions()
                            .iter()
                            .position(|s| s.session_id == r.session_id)
                    })
                    .collect();
            }
        }

        self.rebuild_message_list();
    }

    /// Draw the entire UI.
    fn draw(&self, frame: &mut Frame) {
        let layout = calculate_layout(frame.area(), self.branch_list.len());
        let border_style = Style::default().fg(Color::DarkGray);

        // Left pane border (no right border — shared with right pane)
        let left_block = Block::default()
            .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
            .border_style(border_style);
        frame.render_widget(left_block, layout.left_pane);

        // Right pane border
        let right_block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);
        frame.render_widget(right_block, layout.right_pane);

        // Fix junction points: overwrite right pane corners with proper connectors
        let junction_x = layout.right_pane.x;
        // Top: ┌ → ┬
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("┬", border_style))),
            Rect::new(junction_x, layout.left_pane.y, 1, 1),
        );
        // Bottom: └ → ┴
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("┴", border_style))),
            Rect::new(
                junction_x,
                layout.left_pane.y + layout.left_pane.height.saturating_sub(1),
                1,
                1,
            ),
        );

        // Horizontal separator between search and messages
        let sep1_y = layout.search_area.y + layout.search_area.height;
        self.draw_horizontal_separator(frame, layout.left_pane.x, junction_x, sep1_y, border_style);

        // Horizontal separator between messages and branches
        let sep2_y = layout.branch_area.y.saturating_sub(1);
        self.draw_horizontal_separator(frame, layout.left_pane.x, junction_x, sep2_y, border_style);

        // Search area
        render_search_area(
            frame,
            layout.search_area,
            &self.search_query,
            self.cursor_pos,
            self.visible_message_count(),
            self.total_message_count,
        );

        // Message pane (cross-session list)
        render_message_pane(
            frame,
            layout.message_area,
            &self.message_list,
            self.message_index,
            self.current_branch.as_deref(),
        );

        // Branch pane
        render_branch_pane(
            frame,
            layout.branch_area,
            &self.branch_list,
            self.branch_index,
            self.current_branch.as_deref(),
        );

        // Preview pane
        if let Some(session_id) = self.selected_session_id() {
            let all_messages: Vec<&crate::store::session::Message> = self
                .store
                .sessions()
                .iter()
                .find(|s| s.session_id == session_id)
                .map(|s| s.messages.iter().collect())
                .unwrap_or_default();

            render_preview_pane(
                frame,
                layout.preview_area,
                &all_messages,
                self.preview_mode,
                self.preview_scroll,
            );
        } else {
            render_preview_pane(frame, layout.preview_area, &[], self.preview_mode, 0);
        }

        // Status bar
        self.draw_status_bar(frame, layout.status_bar);
    }

    /// Draw a horizontal separator: ├───...───┤
    fn draw_horizontal_separator(
        &self,
        frame: &mut Frame,
        left_x: u16,
        right_x: u16,
        y: u16,
        style: Style,
    ) {
        let width = right_x - left_x + 1;
        if width < 2 {
            return;
        }
        let mut line_str = String::with_capacity(width as usize * 3);
        line_str.push('├');
        for _ in 1..width - 1 {
            line_str.push('─');
        }
        line_str.push('┤');
        let paragraph = Paragraph::new(Line::from(Span::styled(line_str, style)));
        frame.render_widget(paragraph, Rect::new(left_x, y, width, 1));
    }

    fn draw_status_bar(&self, frame: &mut Frame, area: Rect) {
        let status = if let Some(ref msg) = self.status_message {
            Line::from(Span::styled(
                msg.clone(),
                Style::default().fg(Color::Green).bg(Color::Black),
            ))
        } else {
            let mode_label = match self.preview_mode {
                PreviewMode::Full => "both",
                PreviewMode::Short => "both",
            };
            let msg_info = format!("{} msgs", self.visible_message_count());
            Line::from(vec![
                Span::styled(
                    format!(" {} ", mode_label),
                    Style::default().fg(Color::Black).bg(Color::Cyan),
                ),
                Span::raw(" "),
                Span::styled(msg_info, Style::default().fg(Color::DarkGray)),
                Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "C-j/k:msg  C-n/p:branch  C-d/u:scroll  gg/G:top/btm  Tab:short/full  Enter:resume  C-y:copy",
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        };

        frame.render_widget(Paragraph::new(status), area);
    }

    /// Execute session resume after terminal cleanup.
    pub fn execute_resume(session_id: &str) -> ! {
        let err = Command::new("claude")
            .arg("--resume")
            .arg(session_id)
            .exec();
        eprintln!("Failed to exec claude: {}", err);
        std::process::exit(1);
    }
}
