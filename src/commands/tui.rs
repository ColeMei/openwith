use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Row, Table},
    Frame, Terminal,
};
use std::io;
use std::sync::Mutex;

use crate::core::types::AppInfo;
use crate::core::{duti, scanner, uti};

/// A row in the extension table.
#[derive(Clone)]
struct ExtRow {
    ext: String,
    app_name: String,
    bundle_id: String,
}

/// Which view is active.
enum View {
    /// Main table with optional live filter input active.
    ExtensionList { filtering: bool },
    /// App picker popup.
    AppPicker { filter: String, filtering: bool },
}

struct App {
    view: View,
    all_rows: Vec<ExtRow>,
    filtered_rows: Vec<ExtRow>,
    selected: usize,
    /// Filter string for the extension list.
    filter: String,
    apps: Vec<AppInfo>,
    /// All app names sorted.
    all_app_names: Vec<String>,
    /// Apps shown in picker (may be filtered).
    picker_apps: Vec<String>,
    /// All apps for the current extension (unfiltered source for picker).
    picker_all_apps: Vec<String>,
    picker_selected: usize,
    /// Whether picker is showing all apps or just supporting ones.
    picker_show_all: bool,
    status: String,
    should_quit: bool,
}

impl App {
    fn new(apps: Vec<AppInfo>, rows: Vec<ExtRow>) -> Self {
        let filtered_rows = rows.clone();
        let mut all_app_names: Vec<String> = apps.iter().map(|a| a.name.clone()).collect();
        all_app_names.sort();
        all_app_names.dedup();
        Self {
            view: View::ExtensionList { filtering: false },
            all_rows: rows,
            filtered_rows,
            selected: 0,
            filter: String::new(),
            apps,
            all_app_names,
            picker_apps: Vec::new(),
            picker_all_apps: Vec::new(),
            picker_selected: 0,
            picker_show_all: false,
            status: String::new(),
            should_quit: false,
        }
    }

    fn apply_filter(&mut self) {
        if self.filter.is_empty() {
            self.filtered_rows = self.all_rows.clone();
        } else {
            let f = self.filter.to_lowercase();
            self.filtered_rows = self
                .all_rows
                .iter()
                .filter(|r| {
                    r.ext.to_lowercase().contains(&f) || r.app_name.to_lowercase().contains(&f)
                })
                .cloned()
                .collect();
        }
        if self.selected >= self.filtered_rows.len() {
            self.selected = self.filtered_rows.len().saturating_sub(1);
        }
    }

    fn open_picker(&mut self) {
        if self.filtered_rows.is_empty() {
            return;
        }
        let ext = &self.filtered_rows[self.selected].ext;
        let mut supporting: Vec<String> = self
            .apps
            .iter()
            .filter(|a| a.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
            .map(|a| a.name.clone())
            .collect();
        supporting.sort();
        supporting.dedup();
        self.picker_all_apps = supporting.clone();
        self.picker_apps = supporting;
        self.picker_selected = 0;
        self.picker_show_all = false;
        self.view = View::AppPicker {
            filter: String::new(),
            filtering: false,
        };
    }

    fn picker_toggle_all(&mut self) {
        self.picker_show_all = !self.picker_show_all;
        self.picker_apply_filter(&String::new());
        self.picker_selected = 0;
    }

    fn picker_apply_filter(&mut self, filter: &str) {
        let source = if self.picker_show_all {
            &self.all_app_names
        } else {
            &self.picker_all_apps
        };
        if filter.is_empty() {
            self.picker_apps = source.clone();
        } else {
            let f = filter.to_lowercase();
            self.picker_apps = source
                .iter()
                .filter(|name| name.to_lowercase().contains(&f))
                .cloned()
                .collect();
        }
        if self.picker_selected >= self.picker_apps.len() {
            self.picker_selected = self.picker_apps.len().saturating_sub(1);
        }
    }

    fn set_selected_app(&mut self) {
        if self.picker_apps.is_empty() || self.filtered_rows.is_empty() {
            return;
        }
        let ext = self.filtered_rows[self.selected].ext.clone();
        let app_name = self.picker_apps[self.picker_selected].clone();

        let bundle_id = self
            .apps
            .iter()
            .find(|a| a.name == app_name)
            .and_then(|a| {
                if a.bundle_id.is_empty() {
                    None
                } else {
                    Some(a.bundle_id.clone())
                }
            });

        let bundle_id = match bundle_id {
            Some(id) => id,
            None => {
                self.status = format!("Could not find bundle ID for {}", app_name);
                self.view = View::ExtensionList { filtering: false };
                return;
            }
        };

        match uti::uti_for_extension(&ext) {
            Ok(uti_str) => match duti::set_default(&bundle_id, &uti_str) {
                Ok(_) => {
                    let new_default = duti::query_default(&ext).ok().flatten();
                    if let Some(row) = self.all_rows.iter_mut().find(|r| r.ext == ext) {
                        if let Some(ref d) = new_default {
                            row.app_name = d.name.clone();
                            row.bundle_id = d.bundle_id.clone();
                        }
                    }
                    self.apply_filter();
                    self.status = format!("Set .{} -> {}", ext, app_name);
                }
                Err(e) => {
                    self.status = format!("Failed: {}", e);
                }
            },
            Err(e) => {
                self.status = format!("UTI error: {}", e);
            }
        }
        self.view = View::ExtensionList { filtering: false };
    }

    fn move_down(&mut self) {
        if !self.filtered_rows.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered_rows.len() - 1);
        }
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }
}

pub fn run() -> Result<()> {
    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;

    let mut extensions: Vec<String> = apps
        .iter()
        .flat_map(|app| app.extensions.iter().map(|e| e.to_lowercase()))
        .collect();
    extensions.sort();
    extensions.dedup();

    eprintln!("Querying defaults for {} extensions...", extensions.len());

    let rows: Mutex<Vec<ExtRow>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for chunk in extensions.chunks(20) {
            let rows = &rows;
            let chunk = chunk.to_vec();
            s.spawn(move || {
                for ext in chunk {
                    let default = duti::query_default(&ext).ok().flatten();
                    let (app_name, bundle_id) = match &default {
                        Some(d) => (d.name.clone(), d.bundle_id.clone()),
                        None => ("-".into(), "-".into()),
                    };
                    rows.lock().unwrap().push(ExtRow {
                        ext,
                        app_name,
                        bundle_id,
                    });
                }
            });
        }
    });

    let mut rows = rows.into_inner().unwrap();
    rows.sort_by(|a, b| a.ext.cmp(&b.ext));

    let mut app = App::new(apps, rows);

    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run_loop(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        if app.should_quit {
            break;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match &app.view {
                View::ExtensionList { filtering } => {
                    let filtering = *filtering;
                    handle_list_keys(app, key.code, filtering);
                }
                View::AppPicker { filter, filtering } => {
                    let filter = filter.clone();
                    let filtering = *filtering;
                    handle_picker_keys(app, key.code, &filter, filtering);
                }
            }
        }
    }
    Ok(())
}

fn handle_list_keys(app: &mut App, key: KeyCode, filtering: bool) {
    if filtering {
        // In filter input mode: typing updates filter, arrows navigate list
        match key {
            KeyCode::Esc => {
                app.filter.clear();
                app.apply_filter();
                app.view = View::ExtensionList { filtering: false };
            }
            KeyCode::Enter => {
                // Confirm filter and open picker for selected row
                app.view = View::ExtensionList { filtering: false };
                app.open_picker();
            }
            KeyCode::Backspace => {
                app.filter.pop();
                app.apply_filter();
            }
            KeyCode::Down => app.move_down(),
            KeyCode::Up => app.move_up(),
            KeyCode::Char(c) => {
                app.filter.push(c);
                app.apply_filter();
            }
            _ => {}
        }
    } else {
        // Normal list navigation
        app.status.clear();
        match key {
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => app.move_down(),
            KeyCode::Char('k') | KeyCode::Up => app.move_up(),
            KeyCode::Char('g') | KeyCode::Home => app.selected = 0,
            KeyCode::Char('G') | KeyCode::End => {
                app.selected = app.filtered_rows.len().saturating_sub(1);
            }
            KeyCode::Char('/') => {
                app.view = View::ExtensionList { filtering: true };
            }
            KeyCode::Enter => {
                app.open_picker();
            }
            _ => {}
        }
    }
}

fn handle_picker_keys(app: &mut App, key: KeyCode, filter: &str, filtering: bool) {
    if filtering {
        // Typing to filter the app list
        match key {
            KeyCode::Esc => {
                app.picker_apply_filter(&String::new());
                app.view = View::AppPicker {
                    filter: String::new(),
                    filtering: false,
                };
            }
            KeyCode::Enter => {
                app.view = View::AppPicker {
                    filter: filter.to_string(),
                    filtering: false,
                };
            }
            KeyCode::Backspace => {
                let mut f = filter.to_string();
                f.pop();
                app.picker_apply_filter(&f);
                app.view = View::AppPicker {
                    filter: f,
                    filtering: true,
                };
            }
            KeyCode::Down => {
                if !app.picker_apps.is_empty() {
                    app.picker_selected =
                        (app.picker_selected + 1).min(app.picker_apps.len() - 1);
                }
            }
            KeyCode::Up => {
                app.picker_selected = app.picker_selected.saturating_sub(1);
            }
            KeyCode::Char(c) => {
                let mut f = filter.to_string();
                f.push(c);
                app.picker_apply_filter(&f);
                app.view = View::AppPicker {
                    filter: f,
                    filtering: true,
                };
            }
            _ => {}
        }
    } else {
        match key {
            KeyCode::Esc | KeyCode::Char('q') => {
                app.view = View::ExtensionList { filtering: false };
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !app.picker_apps.is_empty() {
                    app.picker_selected =
                        (app.picker_selected + 1).min(app.picker_apps.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.picker_selected = app.picker_selected.saturating_sub(1);
            }
            KeyCode::Tab => {
                app.picker_toggle_all();
            }
            KeyCode::Char('/') => {
                app.view = View::AppPicker {
                    filter: filter.to_string(),
                    filtering: true,
                };
            }
            KeyCode::Enter => {
                app.set_selected_app();
            }
            _ => {}
        }
    }
}

fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // header + filter
        Constraint::Min(5),   // table
        Constraint::Length(1), // footer
    ])
    .split(f.area());

    draw_header(f, app, chunks[0]);
    draw_table(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);

    if let View::AppPicker { ref filter, filtering } = app.view {
        draw_picker(f, app, filter, filtering);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let filtering = matches!(app.view, View::ExtensionList { filtering: true });

    let filter_display = if app.filter.is_empty() {
        if filtering {
            "_".to_string()
        } else {
            String::new()
        }
    } else if filtering {
        format!("{}_", app.filter)
    } else {
        app.filter.clone()
    };

    let title = format!(
        " openwith  [{} extensions]",
        app.filtered_rows.len()
    );

    let header = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_alignment(ratatui::layout::Alignment::Left);

    let filter_style = if filtering {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let content = if filter_display.is_empty() {
        Paragraph::new(Line::from(vec![
            Span::styled("  /", Style::default().fg(Color::DarkGray)),
            Span::styled(" to filter", Style::default().fg(Color::DarkGray)),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::raw("  Filter: "),
            Span::styled(&filter_display, filter_style),
        ]))
    };

    f.render_widget(content.block(header), area);
}

fn draw_table(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["  EXT", "DEFAULT APP", "BUNDLE ID"])
        .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan));

    let rows: Vec<Row> = app
        .filtered_rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let marker = if i == app.selected { "> " } else { "  " };
            let ext_cell = format!("{}{}", marker, r.ext);
            let style = if i == app.selected {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(vec![ext_cell, r.app_name.clone(), r.bundle_id.clone()]).style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(12),
        Constraint::Percentage(40),
        Constraint::Percentage(45),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT));

    let mut state = ratatui::widgets::TableState::default();
    state.select(Some(app.selected));

    f.render_stateful_widget(table, area, &mut state);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let filtering = matches!(app.view, View::ExtensionList { filtering: true });

    let content = if filtering {
        Line::from(vec![
            Span::styled(" [Enter]", Style::default().fg(Color::Cyan)),
            Span::raw(" confirm  "),
            Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
            Span::raw(" clear  "),
            Span::styled("[Up/Down]", Style::default().fg(Color::Cyan)),
            Span::raw(" navigate"),
        ])
    } else if !app.status.is_empty() {
        Line::from(Span::styled(
            format!(" {}", app.status),
            Style::default().fg(Color::Green),
        ))
    } else {
        Line::from(vec![
            Span::styled(" [Enter]", Style::default().fg(Color::Cyan)),
            Span::raw(" change  "),
            Span::styled("[/]", Style::default().fg(Color::Cyan)),
            Span::raw(" filter  "),
            Span::styled("[q]", Style::default().fg(Color::Cyan)),
            Span::raw(" quit"),
        ])
    };
    f.render_widget(Paragraph::new(content), area);
}

fn draw_picker(f: &mut Frame, app: &App, filter: &str, filtering: bool) {
    let ext = if !app.filtered_rows.is_empty() {
        &app.filtered_rows[app.selected].ext
    } else {
        return;
    };

    let area = f.area();
    let popup_width = 60u16.min(area.width.saturating_sub(4));
    // +4 for borders, +1 for mode/filter line
    let popup_height = (app.picker_apps.len() as u16 + 5)
        .min(area.height.saturating_sub(4))
        .max(7);
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    // Split: mode/filter line at top, then app list filling the rest
    let inner_chunks = Layout::vertical([
        Constraint::Length(1), // mode/filter info line
        Constraint::Min(1),    // app list
    ]);

    let mode_label = if app.picker_show_all {
        "All apps"
    } else {
        "Supporting apps"
    };
    let title = format!(" Set default for .{} ", ext);
    let help = Line::from(vec![
        Span::styled(" [Enter]", Style::default().fg(Color::Cyan)),
        Span::raw(" confirm  "),
        Span::styled("[Tab]", Style::default().fg(Color::Cyan)),
        Span::raw(" all apps  "),
        Span::styled("[/]", Style::default().fg(Color::Cyan)),
        Span::raw(" filter  "),
        Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
        Span::raw(" back "),
    ]);

    let outer_block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_bottom(help);

    let inner_area = outer_block.inner(popup_area);
    f.render_widget(outer_block, popup_area);

    let chunks = inner_chunks.split(inner_area);

    // Mode/filter info line
    let filter_display = if filtering {
        if filter.is_empty() {
            " Filter: _".to_string()
        } else {
            format!(" Filter: {}_", filter)
        }
    } else if !filter.is_empty() {
        format!(" Filter: {}", filter)
    } else {
        String::new()
    };

    let mode_line = if !filter_display.is_empty() {
        Line::from(vec![
            Span::styled(
                format!(" [{}]", mode_label),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                filter_display,
                if filtering {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
        ])
    } else {
        Line::from(vec![Span::styled(
            format!(" [{}]  {} apps", mode_label, app.picker_apps.len()),
            Style::default().fg(Color::DarkGray),
        )])
    };
    f.render_widget(Paragraph::new(mode_line), chunks[0]);

    // App list
    let items: Vec<ListItem> = app
        .picker_apps
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let marker = if i == app.picker_selected {
                "> "
            } else {
                "  "
            };
            let style = if i == app.picker_selected {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{}{}", marker, name)).style(style)
        })
        .collect();

    let list = List::new(items);
    let mut state = ListState::default();
    state.select(Some(app.picker_selected));
    f.render_stateful_widget(list, chunks[1], &mut state);
}
