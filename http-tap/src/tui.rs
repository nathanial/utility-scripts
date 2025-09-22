use std::io;
use std::time::{Duration, SystemTime};

use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Style, Color};
use ratatui::widgets::{Cell, Row, Table, Block, Borders};
use ratatui::Terminal;

use crate::stats::{Aggregator, StatsReceiver, Record};

pub struct App {
    agg: Aggregator,
}

impl App {
    pub fn new() -> Self { Self { agg: Aggregator::default() } }
}

pub async fn run_tui(mut rx: StatsReceiver) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let mut last_redraw = std::time::Instant::now();

    loop {
        // Non-blocking drain of stats
        while let Ok(ev) = rx.try_recv() {
            app.agg.apply(ev);
        }

        // Draw ~30fps max
        if last_redraw.elapsed() > Duration::from_millis(1000 / 30) {
            terminal.draw(|f| {
                let size = f.size();
                let layout = Layout::default()
                    .constraints([Constraint::Percentage(100)])
                    .split(size);

                let rows = app
                    .agg
                    .snapshot()
                    .into_iter()
                    .map(|rec| row_for(&rec));

                let table = Table::new(
                        rows,
                        [
                            Constraint::Percentage(40),
                            Constraint::Length(6),
                            Constraint::Length(6),
                            Constraint::Length(6),
                            Constraint::Length(6),
                            Constraint::Length(6),
                            Constraint::Length(7),
                            Constraint::Percentage(20),
                        ],
                    )
                    .header(
                        Row::new([
                            Cell::from("Path"),
                            Cell::from("GET"),
                            Cell::from("POST"),
                            Cell::from("PUT"),
                            Cell::from("PATCH"),
                            Cell::from("DEL"),
                            Cell::from("OTHER"),
                            Cell::from("Last Seen"),
                        ])
                        .style(Style::default().fg(Color::Yellow)),
                    )
                    .widths(&[])
                    .block(Block::default().borders(Borders::ALL).title("HTTP Tap - q to quit"));

                f.render_widget(table, layout[0]);
            })?;
            last_redraw = std::time::Instant::now();
        }

        // Handle input with timeout to keep UI responsive
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('c') => app.agg = Aggregator::default(),
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn row_for(rec: &Record) -> Row<'static> {
    let last = humanize(rec.last_seen);
    Row::new(vec![
        Cell::from(rec.path.clone()),
        Cell::from(rec.counts.get.to_string()),
        Cell::from(rec.counts.post.to_string()),
        Cell::from(rec.counts.put.to_string()),
        Cell::from(rec.counts.patch.to_string()),
        Cell::from(rec.counts.delete_.to_string()),
        Cell::from(rec.counts.other.to_string()),
        Cell::from(last),
    ])
}

fn humanize(ts: SystemTime) -> String {
    match ts.elapsed() {
        Ok(d) => humantime::format_duration(d).to_string() + " ago",
        Err(_) => "just now".into(),
    }
}
