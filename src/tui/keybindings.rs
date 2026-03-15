use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Actions that can be triggered by key events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    MessageUp,
    MessageDown,
    BranchUp,
    BranchDown,
    ScrollPreviewUp,
    ScrollPreviewDown,
    ScrollPreviewTop,
    ScrollPreviewBottom,
    TogglePreviewMode,
    SearchInput(char),
    SearchBackspace,
    SearchClear,
    Resume,
    CopySessionId,
    None,
}

/// Map a key event to an action.
///
/// Search input is always the default — typing characters adds to the query.
/// Navigation and operations use Ctrl+key or special keys.
pub fn map_key_event(key: KeyEvent, pending_g: bool) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        // Ctrl+c / Ctrl+q / Esc → quit
        KeyCode::Char('c') | KeyCode::Char('q') if ctrl => Action::Quit,
        KeyCode::Esc => Action::Quit,

        // Message navigation: Ctrl+j/k or ↑/↓
        KeyCode::Char('j') if ctrl => Action::MessageDown,
        KeyCode::Char('k') if ctrl => Action::MessageUp,
        KeyCode::Down => Action::MessageDown,
        KeyCode::Up => Action::MessageUp,

        // Branch navigation: Ctrl+n/Ctrl+p
        KeyCode::Char('n') if ctrl => Action::BranchDown,
        KeyCode::Char('p') if ctrl => Action::BranchUp,

        // Preview scroll: Ctrl+d/u → half page
        KeyCode::Char('d') if ctrl => Action::ScrollPreviewDown,
        KeyCode::Char('u') if ctrl => Action::ScrollPreviewUp,

        // gg → top, G → bottom (G = Shift+g)
        KeyCode::Char('g') if pending_g => Action::ScrollPreviewTop,
        KeyCode::Char('G') => Action::ScrollPreviewBottom,

        // Tab → toggle preview mode (short/full)
        KeyCode::Tab => Action::TogglePreviewMode,

        // Enter → resume session
        KeyCode::Enter => Action::Resume,

        // Ctrl+y → copy session ID
        KeyCode::Char('y') if ctrl => Action::CopySessionId,

        // Ctrl+l → clear search
        KeyCode::Char('l') if ctrl => Action::SearchClear,

        // Backspace → delete char from search
        KeyCode::Backspace => Action::SearchBackspace,

        // Regular characters → search input
        KeyCode::Char(c) if !ctrl => Action::SearchInput(c),

        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn shift(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::SHIFT,
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_quit() {
        assert_eq!(map_key_event(ctrl(KeyCode::Char('c')), false), Action::Quit);
        assert_eq!(map_key_event(ctrl(KeyCode::Char('q')), false), Action::Quit);
        assert_eq!(map_key_event(key(KeyCode::Esc), false), Action::Quit);
    }

    #[test]
    fn test_message_navigation() {
        assert_eq!(
            map_key_event(ctrl(KeyCode::Char('j')), false),
            Action::MessageDown
        );
        assert_eq!(
            map_key_event(ctrl(KeyCode::Char('k')), false),
            Action::MessageUp
        );
        assert_eq!(
            map_key_event(key(KeyCode::Down), false),
            Action::MessageDown
        );
        assert_eq!(map_key_event(key(KeyCode::Up), false), Action::MessageUp);
    }

    #[test]
    fn test_branch_navigation() {
        assert_eq!(
            map_key_event(ctrl(KeyCode::Char('n')), false),
            Action::BranchDown
        );
        assert_eq!(
            map_key_event(ctrl(KeyCode::Char('p')), false),
            Action::BranchUp
        );
    }

    #[test]
    fn test_preview_scroll() {
        assert_eq!(
            map_key_event(ctrl(KeyCode::Char('d')), false),
            Action::ScrollPreviewDown
        );
        assert_eq!(
            map_key_event(ctrl(KeyCode::Char('u')), false),
            Action::ScrollPreviewUp
        );
    }

    #[test]
    fn test_gg_and_shift_g() {
        assert_eq!(
            map_key_event(key(KeyCode::Char('g')), false),
            Action::SearchInput('g')
        );
        assert_eq!(
            map_key_event(key(KeyCode::Char('g')), true),
            Action::ScrollPreviewTop
        );
        assert_eq!(
            map_key_event(shift(KeyCode::Char('G')), false),
            Action::ScrollPreviewBottom
        );
    }

    #[test]
    fn test_toggle_preview_mode() {
        assert_eq!(
            map_key_event(key(KeyCode::Tab), false),
            Action::TogglePreviewMode
        );
    }

    #[test]
    fn test_search_input() {
        assert_eq!(
            map_key_event(key(KeyCode::Char('h')), false),
            Action::SearchInput('h')
        );
        assert_eq!(
            map_key_event(key(KeyCode::Char('日')), false),
            Action::SearchInput('日')
        );
        assert_eq!(
            map_key_event(key(KeyCode::Backspace), false),
            Action::SearchBackspace
        );
    }

    #[test]
    fn test_search_clear() {
        assert_eq!(
            map_key_event(ctrl(KeyCode::Char('l')), false),
            Action::SearchClear
        );
    }

    #[test]
    fn test_resume_and_copy() {
        assert_eq!(map_key_event(key(KeyCode::Enter), false), Action::Resume);
        assert_eq!(
            map_key_event(ctrl(KeyCode::Char('y')), false),
            Action::CopySessionId
        );
    }

    #[test]
    fn test_ctrl_chars_not_search_input() {
        assert_eq!(map_key_event(ctrl(KeyCode::Char('a')), false), Action::None);
    }
}
