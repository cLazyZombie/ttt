use crate::app::{App, Mode};
use crate::parser::{LineType, SourceLine};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(frame: &mut Frame, app: &mut App) {
    if app.finished {
        render_results(frame, app);
        return;
    }

    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // code view
            Constraint::Length(1), // status bar
        ])
        .split(area);

    let code_area = chunks[0];
    let status_area = chunks[1];

    match app.mode {
        Mode::Selecting => {
            app.update_scroll_for_select(code_area.height as usize);
            render_select_view(frame, app, code_area);
            render_select_status_bar(frame, app, status_area);
        }
        Mode::Typing => {
            app.update_scroll(code_area.height as usize);
            render_code_view(frame, app, code_area);
            render_status_bar(frame, app, status_area);
        }
    }
}

fn render_code_view(frame: &mut Frame, app: &App, area: Rect) {
    let visible_height = area.height as usize;
    let start = app.scroll_offset;
    let end = (start + visible_height).min(app.source_lines.len());

    let mut lines: Vec<Line> = Vec::new();

    for line_idx in start..end {
        let source_line = &app.source_lines[line_idx];
        let spans = build_line_spans(app, line_idx, source_line);
        lines.push(Line::from(spans));
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).block(Block::default().borders(Borders::NONE));

    frame.render_widget(paragraph, area);
}

fn build_line_spans<'a>(app: &App, line_idx: usize, source_line: &SourceLine) -> Vec<Span<'a>> {
    let mut spans = Vec::new();

    // Line number (dimmed)
    let line_num = format!("{:>4} ", line_idx + 1);
    spans.push(Span::styled(line_num, Style::default().fg(Color::DarkGray)));

    match source_line.line_type {
        LineType::Comment | LineType::Empty => {
            // Show entire line dimmed + italic
            let style = Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC | Modifier::DIM);
            spans.push(Span::styled(source_line.raw.clone(), style));
        }
        LineType::Code | LineType::Mixed => {
            // Leading whitespace (dimmed)
            if !source_line.leading_whitespace.is_empty() {
                spans.push(Span::styled(
                    source_line.leading_whitespace.clone(),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM),
                ));
            }

            // Typeable content - character by character
            render_typeable_content(app, line_idx, source_line, &mut spans);

            // Comment suffix for Mixed lines
            if let Some(ref suffix) = source_line.comment_suffix {
                let style = Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC | Modifier::DIM);
                spans.push(Span::styled(suffix.clone(), style));
            }

            // Per-line speed for completed lines
            if !app.quiet
                && let Some(ref ls) = app.line_stats[line_idx]
            {
                let speed = ls.speed(app.wpm_mode);
                let avg = app.avg_line_speed();
                let color = if avg <= 0.0 {
                    Color::DarkGray
                } else {
                    let ratio = speed / avg;
                    if ratio >= 1.0 {
                        Color::Green
                    } else if ratio >= 0.7 {
                        Color::Yellow
                    } else {
                        Color::Red
                    }
                };
                let label = if app.wpm_mode { "WPM" } else { "KPM" };
                spans.push(Span::styled(
                    format!("  ▐ {:.0} {}", speed, label),
                    Style::default().fg(color),
                ));
            }
        }
    }

    spans
}

fn render_typeable_content<'a>(
    app: &App,
    line_idx: usize,
    source_line: &SourceLine,
    spans: &mut Vec<Span<'a>>,
) {
    let typeable = &source_line.typeable_content;
    let typed = &app.typed_chars[line_idx];
    let is_current_line = line_idx == app.current_line;

    let chars: Vec<char> = typeable.chars().collect();

    for (i, ch) in chars.iter().enumerate() {
        let original_style = source_line
            .typeable_styles
            .get(i)
            .copied()
            .unwrap_or_default();

        let (style, display_char) = if let Some(Some(typed_char)) = typed.get(i) {
            if *typed_char == *ch {
                (original_style, *ch)
            } else {
                (
                    Style::default().fg(Color::White).bg(Color::Red),
                    *typed_char,
                )
            }
        } else if is_current_line && i == app.current_col && !app.finished {
            // Cursor position: reverse video
            (original_style.add_modifier(Modifier::REVERSED), *ch)
        } else {
            // Not yet typed: dimmed
            (original_style.add_modifier(Modifier::DIM), *ch)
        };

        spans.push(Span::styled(display_char.to_string(), style));
    }

    // Show block cursor after the last char when at line end with errors
    if is_current_line && !app.finished && app.current_col >= chars.len() {
        spans.push(Span::styled(
            " ",
            Style::default().add_modifier(Modifier::REVERSED),
        ));
    }
}

fn render_select_view(frame: &mut Frame, app: &App, area: Rect) {
    let visible_height = area.height as usize;
    let start = app.scroll_offset;
    let end = (start + visible_height).min(app.source_lines.len());

    let mut lines: Vec<Line> = Vec::new();

    for line_idx in start..end {
        let source_line = &app.source_lines[line_idx];
        let is_cursor = line_idx == app.select_cursor;

        let line_num = format!("{:>4} ", line_idx + 1);

        let mut spans = Vec::new();

        if is_cursor {
            // Highlighted cursor line
            spans.push(Span::styled(
                line_num,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));

            // Show the full line with a highlight marker
            let line_style = Style::default().bg(Color::DarkGray).fg(Color::White);
            spans.push(Span::styled(source_line.raw.clone(), line_style));
        } else {
            spans.push(Span::styled(line_num, Style::default().fg(Color::DarkGray)));

            // Render tokens with syntax highlighting
            let is_comment = matches!(source_line.line_type, LineType::Comment | LineType::Empty);
            if is_comment {
                let style = Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC | Modifier::DIM);
                spans.push(Span::styled(source_line.raw.clone(), style));
            } else {
                // Show with original styles
                for token in &source_line.tokens {
                    spans.push(Span::styled(token.text.clone(), token.style));
                }
            }
        }

        lines.push(Line::from(spans));
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).block(Block::default().borders(Borders::NONE));
    frame.render_widget(paragraph, area);
}

fn render_select_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let line_info = format!(
        "  Line {}/{}  |  [{}]  |  j/k: move  Enter: start  g/G: top/bottom  Ctrl+C: quit",
        app.select_cursor + 1,
        app.source_lines.len(),
        app.syntax_name,
    );

    let status =
        Paragraph::new(line_info).style(Style::default().fg(Color::Black).bg(Color::Yellow));

    frame.render_widget(status, area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let speed_label = if app.wpm_mode { "WPM" } else { "KPM" };
    let speed_val = app.stats.speed(app.wpm_mode);
    let speed = format!("{speed_label}: {speed_val:.0}");
    let accuracy = format!("Acc: {:.1}%", app.stats.accuracy());
    let progress = format!("Line: {}/{}", app.current_line + 1, app.source_lines.len());
    let time = format!("Time: {}", app.stats.elapsed_display());
    let syntax = format!("[{}]", app.syntax_name);

    let status_text = format!("  {speed}  |  {accuracy}  |  {progress}  |  {time}  |  {syntax}");

    let status =
        Paragraph::new(status_text).style(Style::default().fg(Color::White).bg(Color::DarkGray));

    frame.render_widget(status, area);
}

fn render_results(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let results = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Typing Practice Complete!  ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!(
            "  Final {}:      {:.0}",
            if app.wpm_mode { "WPM" } else { "KPM" },
            app.stats.speed(app.wpm_mode)
        )),
        Line::from(format!("  Accuracy:       {:.1}%", app.stats.accuracy())),
        Line::from(format!("  Total Time:     {}", app.stats.elapsed_display())),
        Line::from(format!("  Keystrokes:     {}", app.stats.total_keystrokes)),
        Line::from(format!("  Lines Typed:    {}", app.total_typeable_lines())),
        Line::from(""),
        Line::from(Span::styled(
            "  Press ESC to exit  ",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(results)
        .block(Block::default().borders(Borders::ALL).title(" Results "))
        .alignment(Alignment::Left);

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Length(12),
            Constraint::Percentage(30),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(vertical[1]);

    frame.render_widget(paragraph, horizontal[1]);
}
