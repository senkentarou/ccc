# ccc (cc collaboration)

Fuzzy search TUI for Claude Code chat history.

## Features

- **3-pane TUI**: Session list, message list, and markdown preview
- **Fuzzy search**: Full-text fuzzy search across all messages using [nucleo](https://github.com/helix-editor/nucleo)
- **Markdown rendering**: Preview with heading, code block, list, and inline code support
- **Session resume**: Press Enter to resume a session with `claude --resume`
- **Clipboard**: Press `y` to copy session ID

## Install

```bash
cargo install --path .
```

## Usage

```bash
# Run in a project directory where you've used Claude Code
ccc

# Or specify a project path
ccc --path /path/to/project
```

## Keybindings

| Key | Action |
|-----|--------|
| `↑/k` | Move up |
| `↓/j` | Move down |
| `Tab` | Switch pane |
| `Shift+Tab` | Switch pane (reverse) |
| `/` | Start search |
| `Esc` | Exit search |
| `u/a/b` | Filter: user / assistant / both |
| `Enter` | Resume session |
| `y` | Copy session ID |
| `Ctrl+u/d` | Scroll preview up/down |
| `q` | Quit |

## License

MIT
