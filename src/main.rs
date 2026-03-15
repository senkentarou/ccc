mod app;
mod parser;
mod render;
mod search;
mod store;
mod tui;

use anyhow::{Context, Result};
use clap::Parser;
use std::env;
use std::process::Command;

use app::App;
use store::session::SessionStore;

/// ccc - Claude Code Collaboration
///
/// Fuzzy search TUI for Claude Code chat history.
/// Searches sessions in the current project directory.
#[derive(Parser, Debug)]
#[command(name = "ccc", version, about)]
struct Cli {
    /// Project path to search (defaults to current directory)
    #[arg(short, long)]
    path: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let project_path = cli
        .path
        .or_else(|| {
            env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        })
        .context("Could not determine project path")?;

    let store = SessionStore::load(&project_path)?;

    if store.is_empty() {
        eprintln!("No sessions found for project: {project_path}");
        eprintln!(
            "Make sure you're running ccc from a project directory where you've used Claude Code."
        );
        return Ok(());
    }

    let current_branch = detect_current_branch();
    let mut app = App::new(store, current_branch);

    let mut terminal = ratatui::init();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.run(&mut terminal)));
    ratatui::restore();

    match result {
        Ok(inner) => inner?,
        Err(panic) => std::panic::resume_unwind(panic),
    }

    // If resume was requested, exec claude after terminal cleanup
    if let Some(session_id) = app.resume_session_id {
        eprintln!(
            "Resuming session {}...",
            &session_id[..8.min(session_id.len())]
        );
        App::execute_resume(&session_id);
    }

    Ok(())
}

/// Detect the current git branch by running `git rev-parse --abbrev-ref HEAD`.
fn detect_current_branch() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let branch = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if branch.is_empty() {
                None
            } else {
                Some(branch)
            }
        })
}
