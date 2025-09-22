use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::app::App;

pub fn run(app: &mut App) -> Result<()> {
    if app.is_empty() {
        app.cancel();
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = loop {
        terminal.draw(|frame| crate::ui::draw(frame, app))?;

        if app.should_quit() {
            break Ok(());
        }

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                handle_key_event(app, key);
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn handle_key_event(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.cancel(),
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::Char(' ') => {
            app.toggle_current();
            app.clear_message();
        }
        KeyCode::Char('a') => {
            app.toggle_all();
            app.clear_message();
        }
        KeyCode::Enter => app.confirm(),
        _ => {}
    }
}
