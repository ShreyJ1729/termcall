use std::io::{self, stdout, Stdout};

use crossterm::{execute, terminal::*};
use ratatui::prelude::*;

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal
pub fn init() -> io::Result<Tui> {
    let mut t = Terminal::new(CrosstermBackend::new(stdout()))?;
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    t.hide_cursor()?;

    Ok(t)
}

/// Restore the terminal to its original state
pub fn restore() -> io::Result<()> {
    let mut t = Terminal::new(CrosstermBackend::new(stdout()))?;
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    t.show_cursor()?;
    Ok(())
}
