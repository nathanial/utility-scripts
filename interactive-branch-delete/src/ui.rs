use humantime::format_duration;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::app::App;

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let size = frame.size();

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
        .split(size);

    let mut state = ListState::default();
    if !app.is_empty() {
        state.select(Some(app.cursor()));
    }

    let list_items: Vec<ListItem> = app
        .items()
        .iter()
        .map(|branch| {
            let marker = if branch.selected { "[x]" } else { "[ ]" };
            let status_span = if branch.info.merged {
                Span::styled("merged", Style::default().fg(Color::Green))
            } else {
                Span::styled("unmerged", Style::default().fg(Color::Red))
            };
            let age_span = branch.age.map(|age| {
                Span::styled(
                    format_duration(age).to_string(),
                    Style::default().fg(Color::Magenta),
                )
            });
            let summary = branch
                .info
                .summary
                .as_deref()
                .unwrap_or("<no commit message>");
            let mut spans = vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(&branch.info.name, Style::default().fg(Color::Yellow)),
                Span::raw("  "),
                status_span,
            ];
            if let Some(age_span) = age_span {
                spans.push(Span::raw("  "));
                spans.push(age_span);
            }
            spans.push(Span::raw("  "));
            spans.push(Span::raw(summary));

            let primary = Line::from(spans);
            ListItem::new(primary)
        })
        .collect();

    let title = format!(
        "Branches relative to '{}' (current: {}) - {} / {} selected",
        app.base_branch(),
        app.current_branch(),
        app.selected_count(),
        app.total_count()
    );

    let list = List::new(list_items)
        .block(
            Block::default()
                .title(Span::styled(
                    title,
                    Style::default().add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, vertical[0], &mut state);

    let help_line = "up/down or j/k: move  space: toggle  a: toggle all  enter: confirm  q: cancel";
    let status_line = app
        .message()
        .map(ToString::to_string)
        .unwrap_or_else(|| "Select branches to delete.".to_string());

    let status_block = Paragraph::new(vec![Line::from(help_line), Line::from(status_line)])
        .block(Block::default().title("Status").borders(Borders::ALL));

    frame.render_widget(status_block, vertical[1]);
}
