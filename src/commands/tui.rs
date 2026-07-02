use anyhow::Result;
use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Row, Table},
};
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::core::types::AppInfo;
use crate::core::{launchservices, scanner, uti};
use crate::logo::LOGO;

/// Which view to show when the TUI starts.
pub enum InitialView {
    Extensions,
    Apps,
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A row in the extension table.
#[derive(Clone)]
struct ExtRow {
    ext: String,
    app_name: String,
    bundle_id: String,
}

/// An entry in the apps browser.
#[derive(Clone)]
struct AppBrowserEntry {
    name: String,
    bundle_id: String,
    supported: Vec<String>,
    default_for: Vec<String>,
}

/// Which top-level tab is active.
#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Extensions,
    Apps,
}

/// Which view / overlay is active.
enum View {
    ExtensionList { filtering: bool },
    AppPicker { filter: String, filtering: bool },
    AppsBrowser { filtering: bool },
    Help,
}

enum StatusKind {
    Success,
    Error,
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

struct LoadResult {
    apps: Vec<AppInfo>,
    rows: Vec<ExtRow>,
}

enum LoadPhase {
    Scanning,
    Querying { total: usize },
    Done(Option<LoadResult>),
    Error(String),
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct App {
    tab: Tab,
    view: View,

    // Extension list state
    all_rows: Vec<ExtRow>,
    filtered_rows: Vec<ExtRow>,
    selected: usize,
    filter: String,

    // Shared app data
    apps: Vec<AppInfo>,
    all_app_names: Vec<String>,

    // App picker state
    picker_apps: Vec<String>,
    picker_all_apps: Vec<String>,
    picker_selected: usize,
    picker_show_all: bool,

    // Apps browser state
    apps_entries: Vec<AppBrowserEntry>,
    apps_filtered: Vec<usize>,
    apps_filter: String,
    apps_selected: usize,

    // Status
    status: String,
    status_kind: StatusKind,

    // Change tracking
    changes: Vec<(String, String)>,

    should_quit: bool,
}

impl App {
    fn new(apps: Vec<AppInfo>, rows: Vec<ExtRow>, initial_tab: Tab) -> Self {
        let filtered_rows = rows.clone();
        let mut all_app_names: Vec<String> = apps.iter().map(|a| a.name.clone()).collect();
        all_app_names.sort();
        all_app_names.dedup();

        // Build apps browser entries
        let apps_entries = Self::build_apps_entries(&apps, &rows);
        let apps_filtered: Vec<usize> = (0..apps_entries.len()).collect();

        let view = match initial_tab {
            Tab::Extensions => View::ExtensionList { filtering: false },
            Tab::Apps => View::AppsBrowser { filtering: false },
        };

        Self {
            tab: initial_tab,
            view,
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
            apps_entries,
            apps_filtered,
            apps_filter: String::new(),
            apps_selected: 0,
            status: String::new(),
            status_kind: StatusKind::Success,
            changes: Vec::new(),
            should_quit: false,
        }
    }

    fn build_apps_entries(apps: &[AppInfo], rows: &[ExtRow]) -> Vec<AppBrowserEntry> {
        let mut entries: Vec<AppBrowserEntry> = apps
            .iter()
            .filter(|a| !a.bundle_id.is_empty())
            .map(|a| {
                let mut supported = a.extensions.clone();
                supported.extend(
                    rows.iter()
                        .filter(|row| scanner::app_supports_extension(a, &row.ext))
                        .map(|row| row.ext.clone()),
                );
                supported.sort();
                supported.dedup();
                let default_for: Vec<String> = supported
                    .iter()
                    .filter(|ext| {
                        rows.iter().any(|r| {
                            r.ext.eq_ignore_ascii_case(ext)
                                && r.bundle_id.eq_ignore_ascii_case(&a.bundle_id)
                        })
                    })
                    .cloned()
                    .collect();
                AppBrowserEntry {
                    name: a.name.clone(),
                    bundle_id: a.bundle_id.clone(),
                    supported,
                    default_for,
                }
            })
            .filter(|a| !a.supported.is_empty())
            .collect();
        entries.sort_by_key(|a| a.name.to_lowercase());
        entries
    }

    // --- Extension list ---

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
            .filter(|a| scanner::app_supports_extension(a, ext))
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
        self.picker_apply_filter("");
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

        let resolved_app = match scanner::resolve_app(&self.apps, &app_name) {
            Ok(app) if !app.bundle_id.is_empty() => app,
            Ok(app) => {
                self.status = format!("Could not determine bundle ID for {}", app.name);
                self.status_kind = StatusKind::Error;
                self.view = View::ExtensionList { filtering: false };
                return;
            }
            Err(err) => {
                self.status = err.to_string();
                self.status_kind = StatusKind::Error;
                self.view = View::ExtensionList { filtering: false };
                return;
            }
        };

        let bundle_id = resolved_app.bundle_id.clone();

        match uti::uti_for_extension(&ext) {
            Ok(uti_str) => match launchservices::set_default(&bundle_id, &uti_str) {
                Ok(_) => {
                    let new_bid = launchservices::query_default_bundle_id(&ext).ok().flatten();
                    let verified = new_bid
                        .as_ref()
                        .map(|b| b.eq_ignore_ascii_case(&bundle_id))
                        .unwrap_or(false);

                    // The default follows the UTI, so every extension sharing
                    // it changed too — update their rows as well.
                    let siblings = uti::extensions_sharing_uti(
                        &ext,
                        &uti_str,
                        &scanner::all_extensions(&self.apps),
                    );

                    let previous_name = self
                        .all_rows
                        .iter()
                        .find(|r| r.ext == ext)
                        .map(|r| r.app_name.clone())
                        .filter(|n| n != "-" && !n.eq_ignore_ascii_case(&app_name));

                    if let Some(ref bid) = new_bid {
                        let mut affected = vec![ext.clone()];
                        affected.extend(siblings.iter().cloned());
                        for affected_ext in &affected {
                            let Some(row_idx) = self
                                .all_rows
                                .iter()
                                .position(|r| r.ext.eq_ignore_ascii_case(affected_ext))
                            else {
                                continue;
                            };
                            let old_bid = self.all_rows[row_idx].bundle_id.clone();
                            self.all_rows[row_idx].app_name =
                                scanner::resolve_name(&self.apps, bid);
                            self.all_rows[row_idx].bundle_id = bid.clone();

                            // Update apps browser entries
                            self.update_apps_browser_default(affected_ext, &old_bid, bid);
                        }
                    }
                    self.apply_filter();

                    if verified {
                        let mut status = format!("Set .{} -> {}", ext, app_name);
                        if let Some(prev) = previous_name {
                            status.push_str(&format!(" (was {prev})"));
                        }
                        if !siblings.is_empty() {
                            let shown: Vec<String> =
                                siblings.iter().take(3).map(|s| format!(".{s}")).collect();
                            let extra = siblings.len().saturating_sub(3);
                            let more = if extra > 0 {
                                format!(" +{extra}")
                            } else {
                                String::new()
                            };
                            status.push_str(&format!(
                                " (also affects {}{})",
                                shown.join(", "),
                                more
                            ));
                        }
                        self.status = status;
                        self.status_kind = StatusKind::Success;
                        self.changes.push((ext, app_name));
                    } else {
                        self.status = format!("Set .{} -> {} (could not verify)", ext, app_name);
                        self.status_kind = StatusKind::Error;
                    }
                }
                Err(e) => {
                    self.status = format!("Failed: {}", e);
                    self.status_kind = StatusKind::Error;
                }
            },
            Err(e) => {
                self.status = format!("UTI error: {}", e);
                self.status_kind = StatusKind::Error;
            }
        }
        self.view = View::ExtensionList { filtering: false };
    }

    fn update_apps_browser_default(&mut self, ext: &str, old_bid: &str, new_bid: &str) {
        // Remove ext from old app's default_for
        if let Some(old_entry) = self
            .apps_entries
            .iter_mut()
            .find(|e| e.bundle_id.eq_ignore_ascii_case(old_bid))
        {
            old_entry
                .default_for
                .retain(|e| !e.eq_ignore_ascii_case(ext));
        }
        // Add ext to new app's default_for
        if let Some(new_entry) = self
            .apps_entries
            .iter_mut()
            .find(|e| e.bundle_id.eq_ignore_ascii_case(new_bid))
            && !new_entry
                .default_for
                .iter()
                .any(|e| e.eq_ignore_ascii_case(ext))
        {
            new_entry.default_for.push(ext.to_string());
            new_entry.default_for.sort();
        }
    }

    fn move_down(&mut self) {
        if !self.filtered_rows.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered_rows.len() - 1);
        }
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    // --- Apps browser ---

    fn apps_apply_filter(&mut self) {
        if self.apps_filter.is_empty() {
            self.apps_filtered = (0..self.apps_entries.len()).collect();
        } else {
            let f = self.apps_filter.to_lowercase();
            self.apps_filtered = self
                .apps_entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.name.to_lowercase().contains(&f))
                .map(|(i, _)| i)
                .collect();
        }
        if self.apps_selected >= self.apps_filtered.len() {
            self.apps_selected = self.apps_filtered.len().saturating_sub(1);
        }
    }

    fn apps_move_down(&mut self) {
        if !self.apps_filtered.is_empty() {
            self.apps_selected = (self.apps_selected + 1).min(self.apps_filtered.len() - 1);
        }
    }

    fn apps_move_up(&mut self) {
        self.apps_selected = self.apps_selected.saturating_sub(1);
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Restores the terminal on drop, so every exit path (including `?` early
/// returns) leaves the user's shell usable.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        io::stdout().execute(EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = io::stdout().execute(LeaveAlternateScreen);
}

/// Restore the terminal before the default panic output runs, so the panic
/// message is readable instead of vanishing with the alternate screen.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));
}

pub fn run(initial_view: InitialView) -> Result<()> {
    let initial_tab = match initial_view {
        InitialView::Extensions => Tab::Extensions,
        InitialView::Apps => Tab::Apps,
    };

    // Enter TUI immediately, scan in background
    install_panic_hook();
    let guard = TerminalGuard::enter()?;
    let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    // Shared loading state
    let phase = Arc::new(Mutex::new(LoadPhase::Scanning));
    let progress = Arc::new(AtomicUsize::new(0));

    // Spawn background loader
    let phase_clone = Arc::clone(&phase);
    let progress_clone = Arc::clone(&progress);
    std::thread::spawn(move || {
        let result = load_data(phase_clone, progress_clone);
        // If load_data didn't set Done/Error, that means it panicked or errored
        // before reaching those points — handled by the main loop checking Error.
        drop(result);
    });

    // Loading render loop
    let mut spinner_frame: usize = 0;
    let mut quit_during_load = false;
    let mut load_result: Option<LoadResult> = None;

    loop {
        // Draw loading screen
        let (phase_text, done) = {
            let p = phase.lock().unwrap();
            match &*p {
                LoadPhase::Scanning => ("Scanning applications...".to_string(), false),
                LoadPhase::Querying { total } => {
                    let done_count = progress.load(Ordering::Relaxed);
                    (
                        format!("Querying defaults ({}/{})...", done_count, total),
                        false,
                    )
                }
                LoadPhase::Done(_) => (String::new(), true),
                LoadPhase::Error(e) => (format!("Error: {}", e), false),
            }
        };

        terminal.draw(|f| {
            draw_loading(f, &phase_text, spinner_frame);
        })?;

        if done {
            // Take the result out
            let mut p = phase.lock().unwrap();
            if let LoadPhase::Done(ref mut opt) = *p {
                load_result = opt.take();
            } else {
                load_result = None;
            }
            break;
        }

        // Poll for quit key
        if event::poll(Duration::from_millis(80))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && key.code == KeyCode::Char('q')
        {
            quit_during_load = true;
            break;
        }
        spinner_frame = (spinner_frame + 1) % SPINNER.len();
    }

    if quit_during_load {
        drop(guard);
        println!("Goodbye! \u{2014} openwith v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let Some(data) = load_result else {
        drop(guard);
        anyhow::bail!("Failed to load application data");
    };

    let mut app = App::new(data.apps, data.rows, initial_tab);

    let result = run_loop(&mut terminal, &mut app);

    drop(guard);

    // Exit summary
    if !app.changes.is_empty() {
        let count = app.changes.len();
        println!(
            "Changed {} default{}:",
            count,
            if count == 1 { "" } else { "s" }
        );
        for (ext, name) in &app.changes {
            println!("  .{} \u{2192} {}", ext, name);
        }
    }
    println!("Goodbye! \u{2014} openwith v{}", env!("CARGO_PKG_VERSION"));

    result
}

fn load_data(phase: Arc<Mutex<LoadPhase>>, progress: Arc<AtomicUsize>) -> Result<()> {
    let apps = match scanner::scan_all_apps() {
        Ok(a) => a,
        Err(e) => {
            *phase.lock().unwrap() = LoadPhase::Error(e.to_string());
            return Err(e);
        }
    };

    let extensions = scanner::all_extensions(&apps);

    let total = extensions.len();
    *phase.lock().unwrap() = LoadPhase::Querying { total };

    let rows: Mutex<Vec<ExtRow>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for chunk in extensions.chunks(20) {
            let rows = &rows;
            let apps = &apps;
            let progress = &progress;
            let chunk = chunk.to_vec();
            s.spawn(move || {
                for ext in chunk {
                    let bundle_id = launchservices::query_default_bundle_id(&ext).ok().flatten();
                    let (app_name, bid) = match &bundle_id {
                        Some(bid) => (scanner::resolve_name(apps, bid), bid.clone()),
                        None => ("-".into(), "-".into()),
                    };
                    rows.lock().unwrap().push(ExtRow {
                        ext,
                        app_name,
                        bundle_id: bid,
                    });
                    progress.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });

    let mut rows = rows.into_inner().unwrap();
    rows.sort_by(|a, b| a.ext.cmp(&b.ext));

    *phase.lock().unwrap() = LoadPhase::Done(Some(LoadResult { apps, rows }));
    Ok(())
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

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
                View::AppsBrowser { filtering } => {
                    let filtering = *filtering;
                    handle_apps_browser_keys(app, key.code, filtering);
                }
                View::Help => {
                    // Any key dismisses help, return to previous tab view
                    match app.tab {
                        Tab::Extensions => {
                            app.view = View::ExtensionList { filtering: false };
                        }
                        Tab::Apps => {
                            app.view = View::AppsBrowser { filtering: false };
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Key handlers
// ---------------------------------------------------------------------------

fn handle_list_keys(app: &mut App, key: KeyCode, filtering: bool) {
    if filtering {
        match key {
            KeyCode::Esc => {
                app.filter.clear();
                app.apply_filter();
                app.view = View::ExtensionList { filtering: false };
            }
            KeyCode::Enter => {
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
            KeyCode::Char('?') => {
                app.view = View::Help;
            }
            KeyCode::Enter => {
                app.open_picker();
            }
            KeyCode::Tab => {
                app.tab = Tab::Apps;
                app.view = View::AppsBrowser { filtering: false };
            }
            _ => {}
        }
    }
}

fn handle_picker_keys(app: &mut App, key: KeyCode, filter: &str, filtering: bool) {
    if filtering {
        match key {
            KeyCode::Esc => {
                app.picker_apply_filter("");
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
            KeyCode::Down if !app.picker_apps.is_empty() => {
                app.picker_selected = (app.picker_selected + 1).min(app.picker_apps.len() - 1);
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
            KeyCode::Char('j') | KeyCode::Down if !app.picker_apps.is_empty() => {
                app.picker_selected = (app.picker_selected + 1).min(app.picker_apps.len() - 1);
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

fn handle_apps_browser_keys(app: &mut App, key: KeyCode, filtering: bool) {
    if filtering {
        match key {
            KeyCode::Esc => {
                app.apps_filter.clear();
                app.apps_apply_filter();
                app.view = View::AppsBrowser { filtering: false };
            }
            KeyCode::Enter => {
                app.view = View::AppsBrowser { filtering: false };
            }
            KeyCode::Backspace => {
                app.apps_filter.pop();
                app.apps_apply_filter();
            }
            KeyCode::Down => app.apps_move_down(),
            KeyCode::Up => app.apps_move_up(),
            KeyCode::Char(c) => {
                app.apps_filter.push(c);
                app.apps_apply_filter();
            }
            _ => {}
        }
    } else {
        app.status.clear();
        match key {
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => app.apps_move_down(),
            KeyCode::Char('k') | KeyCode::Up => app.apps_move_up(),
            KeyCode::Char('g') | KeyCode::Home => app.apps_selected = 0,
            KeyCode::Char('G') | KeyCode::End => {
                app.apps_selected = app.apps_filtered.len().saturating_sub(1);
            }
            KeyCode::Char('/') => {
                app.view = View::AppsBrowser { filtering: true };
            }
            KeyCode::Char('?') => {
                app.view = View::Help;
            }
            KeyCode::Tab => {
                app.tab = Tab::Extensions;
                app.view = View::ExtensionList { filtering: false };
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // header + filter/tabs
        Constraint::Min(5),    // content
        Constraint::Length(1), // footer
    ])
    .split(f.area());

    draw_header(f, app, chunks[0]);

    match app.tab {
        Tab::Extensions => draw_table(f, app, chunks[1]),
        Tab::Apps => draw_apps_browser(f, app, chunks[1]),
    }

    draw_footer(f, app, chunks[2]);

    match &app.view {
        View::AppPicker { filter, filtering } => {
            draw_picker(f, app, filter, *filtering);
        }
        View::Help => {
            draw_help(f);
        }
        _ => {}
    }
}

fn draw_loading(f: &mut Frame, phase_text: &str, spinner_frame: usize) {
    let area = f.area();

    let logo_lines: Vec<&str> = LOGO.lines().collect();
    let logo_height = logo_lines.len() as u16;
    let total_height = logo_height + 3; // logo + blank + spinner line
    let start_y = area.height.saturating_sub(total_height) / 2;

    // Draw logo
    let logo_spans: Vec<Line> = logo_lines
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(Color::Cyan))))
        .collect();

    let logo_width = logo_lines
        .iter()
        .map(|l| l.len() as u16)
        .max()
        .unwrap_or(40);
    let logo_x = area.width.saturating_sub(logo_width) / 2;
    let logo_area = Rect::new(logo_x, start_y, logo_width.min(area.width), logo_height);
    f.render_widget(Paragraph::new(logo_spans), logo_area);

    // Draw spinner + phase text
    if !phase_text.is_empty() {
        let spinner_char = SPINNER[spinner_frame % SPINNER.len()];
        let spinner_text = format!("{} {}", spinner_char, phase_text);
        let spinner_width = spinner_text.len() as u16;
        let spinner_x = area.width.saturating_sub(spinner_width) / 2;
        let spinner_y = start_y + logo_height + 1;
        if spinner_y < area.height {
            let spinner_area = Rect::new(spinner_x, spinner_y, spinner_width.min(area.width), 1);
            f.render_widget(
                Paragraph::new(Span::styled(
                    spinner_text,
                    Style::default().fg(Color::Yellow),
                )),
                spinner_area,
            );
        }
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    // Build tab line
    let ext_style = if app.tab == Tab::Extensions {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let apps_style = if app.tab == Tab::Apps {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let ext_label = if app.tab == Tab::Extensions {
        " [Extensions] "
    } else {
        "  Extensions  "
    };
    let apps_label = if app.tab == Tab::Apps {
        " [Apps] "
    } else {
        "  Apps  "
    };

    let count_text = match app.tab {
        Tab::Extensions => format!(" [{} extensions]", app.filtered_rows.len()),
        Tab::Apps => format!(" [{} apps]", app.apps_filtered.len()),
    };

    let title = Line::from(vec![
        Span::styled(ext_label, ext_style),
        Span::styled(apps_label, apps_style),
        Span::styled(count_text, Style::default().fg(Color::DarkGray)),
    ]);

    let header = Block::default().borders(Borders::ALL).title(title);

    // Filter display depends on active view
    let (filter_text, is_filtering) = match (&app.view, app.tab) {
        (View::ExtensionList { filtering: true }, Tab::Extensions) => {
            let display = if app.filter.is_empty() {
                "_".to_string()
            } else {
                format!("{}_", app.filter)
            };
            (display, true)
        }
        (View::ExtensionList { .. }, Tab::Extensions) if !app.filter.is_empty() => {
            (app.filter.clone(), false)
        }
        (View::AppsBrowser { filtering: true }, Tab::Apps) => {
            let display = if app.apps_filter.is_empty() {
                "_".to_string()
            } else {
                format!("{}_", app.apps_filter)
            };
            (display, true)
        }
        (View::AppsBrowser { .. }, Tab::Apps) if !app.apps_filter.is_empty() => {
            (app.apps_filter.clone(), false)
        }
        _ => (String::new(), false),
    };

    let filter_style = if is_filtering {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let content = if filter_text.is_empty() {
        Paragraph::new(Line::from(vec![
            Span::styled("  /", Style::default().fg(Color::DarkGray)),
            Span::styled(" to filter", Style::default().fg(Color::DarkGray)),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::raw("  Filter: "),
            Span::styled(&filter_text, filter_style),
        ]))
    };

    f.render_widget(content.block(header), area);
}

fn draw_table(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["  EXT", "DEFAULT APP", "BUNDLE ID"]).style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Cyan),
    );

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

fn draw_apps_browser(f: &mut Frame, app: &App, area: Rect) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)]).split(area);

    // Left pane: app list
    let items: Vec<ListItem> = app
        .apps_filtered
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            let entry = &app.apps_entries[idx];
            let marker = if i == app.apps_selected { "> " } else { "  " };
            let style = if i == app.apps_selected {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{}{}", marker, entry.name)).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
            .title(Span::styled(
                " Apps ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
    );
    let mut list_state = ListState::default();
    list_state.select(Some(app.apps_selected));
    f.render_stateful_widget(list, chunks[0], &mut list_state);

    // Right pane: detail view
    let detail_block = Block::default()
        .borders(Borders::RIGHT | Borders::BOTTOM)
        .title(Span::styled(
            " Details ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    if app.apps_filtered.is_empty() {
        let empty = Paragraph::new(Span::styled(
            "  No apps found",
            Style::default().fg(Color::DarkGray),
        ))
        .block(detail_block);
        f.render_widget(empty, chunks[1]);
        return;
    }

    let idx = app.apps_filtered[app.apps_selected];
    let entry = &app.apps_entries[idx];

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                &entry.name,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(&entry.bundle_id, Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("Supported extensions ({}):", entry.supported.len()),
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ];

    // Wrap supported extensions to fit the pane width
    let detail_width = chunks[1].width.saturating_sub(6) as usize; // padding
    for line_text in wrap_list(&entry.supported, detail_width) {
        lines.push(Line::from(format!("    {}", line_text)));
    }

    lines.push(Line::from(""));

    if entry.default_for.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Not the default for any extension",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("Default for ({}):", entry.default_for.len()),
                Style::default().fg(Color::Green),
            ),
        ]));
        for line_text in wrap_list(&entry.default_for, detail_width) {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(line_text, Style::default().fg(Color::Green)),
            ]));
        }
    }

    let detail = Paragraph::new(lines).block(detail_block);
    f.render_widget(detail, chunks[1]);
}

/// Wrap a list of items as comma-separated lines fitting within `max_width`.
fn wrap_list(items: &[String], max_width: usize) -> Vec<String> {
    if items.is_empty() {
        return vec!["(none)".to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for (i, item) in items.iter().enumerate() {
        let sep = if i == 0 { "" } else { ", " };
        if !current.is_empty() && current.len() + sep.len() + item.len() > max_width {
            lines.push(current);
            current = item.clone();
        } else {
            current.push_str(sep);
            current.push_str(item);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let is_filtering = matches!(
        (&app.view, app.tab),
        (View::ExtensionList { filtering: true }, Tab::Extensions)
            | (View::AppsBrowser { filtering: true }, Tab::Apps)
    );

    let content = if is_filtering {
        Line::from(vec![
            Span::styled(" [Enter]", Style::default().fg(Color::Cyan)),
            Span::raw(" confirm  "),
            Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
            Span::raw(" clear  "),
            Span::styled("[Up/Down]", Style::default().fg(Color::Cyan)),
            Span::raw(" navigate"),
        ])
    } else if !app.status.is_empty() {
        let color = match app.status_kind {
            StatusKind::Success => Color::Green,
            StatusKind::Error => Color::Red,
        };
        Line::from(Span::styled(
            format!(" {}", app.status),
            Style::default().fg(color),
        ))
    } else {
        match app.tab {
            Tab::Extensions => Line::from(vec![
                Span::styled(" [Enter]", Style::default().fg(Color::Cyan)),
                Span::raw(" change  "),
                Span::styled("[/]", Style::default().fg(Color::Cyan)),
                Span::raw(" filter  "),
                Span::styled("[Tab]", Style::default().fg(Color::Cyan)),
                Span::raw(" apps  "),
                Span::styled("[?]", Style::default().fg(Color::Cyan)),
                Span::raw(" help  "),
                Span::styled("[q]", Style::default().fg(Color::Cyan)),
                Span::raw(" quit"),
            ]),
            Tab::Apps => Line::from(vec![
                Span::styled(" [/]", Style::default().fg(Color::Cyan)),
                Span::raw(" filter  "),
                Span::styled("[Tab]", Style::default().fg(Color::Cyan)),
                Span::raw(" extensions  "),
                Span::styled("[?]", Style::default().fg(Color::Cyan)),
                Span::raw(" help  "),
                Span::styled("[q]", Style::default().fg(Color::Cyan)),
                Span::raw(" quit"),
            ]),
        }
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
    let popup_height = (app.picker_apps.len() as u16 + 5)
        .min(area.height.saturating_sub(4))
        .max(7);
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let inner_chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]);

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

    let items: Vec<ListItem> = app
        .picker_apps
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let marker = if i == app.picker_selected { "> " } else { "  " };
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

fn draw_help(f: &mut Frame) {
    let width = 76u16;
    let height = 22u16;
    let area = f.area();
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width.min(area.width), height.min(area.height));

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Keyboard Shortcuts ")
        .title_bottom(Line::from(Span::styled(
            " Press any key to close ",
            Style::default().fg(Color::DarkGray),
        )));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Three-part horizontal layout: left | divider | right
    let cols = Layout::horizontal([
        Constraint::Percentage(48),
        Constraint::Length(1),
        Constraint::Percentage(48),
    ])
    .split(inner);

    // Draw vertical divider
    let divider_lines: Vec<Line> = (0..cols[1].height)
        .map(|_| Line::from(Span::styled("│", Style::default().fg(Color::DarkGray))))
        .collect();
    f.render_widget(Paragraph::new(divider_lines), cols[1]);

    // Left column: Extension List + App Picker
    let left_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "   Extension List",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        help_line("j/k  Up/Down", "Navigate"),
        help_line("g / G", "Top / bottom"),
        help_line("/", "Filter"),
        help_line("Enter", "App picker"),
        help_line("Tab", "Switch tab"),
        help_line("?", "Help"),
        help_line("q", "Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "   App Picker",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        help_line("Tab", "All / supporting"),
        help_line("/", "Filter"),
        help_line("Enter", "Confirm"),
        help_line("Esc", "Close"),
    ];

    // Right column: Apps Browser
    let right_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "   Apps Browser",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        help_line("j/k  Up/Down", "Navigate"),
        help_line("g / G", "Top / bottom"),
        help_line("/", "Filter"),
        help_line("Tab", "Switch tab"),
        help_line("?", "Help"),
        help_line("q", "Quit"),
    ];

    f.render_widget(Paragraph::new(left_lines), cols[0]);
    f.render_widget(Paragraph::new(right_lines), cols[2]);
}

fn help_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("  {:>14}", key),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  {}", desc)),
    ])
}
