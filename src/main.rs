mod audio;
mod playlist;
mod browser;
mod config;

use audio::AudioEngine;
use playlist::{Playlist, RepeatMode};
use browser::FileBrowser;
use config::Config;
use lofty::{probe::Probe, prelude::Accessor, file::TaggedFileExt};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Gauge, ListState, Clear, Wrap},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver};
use std::thread;

enum Modal {
    None,
    Help,
    Settings,
    SavePlaylist,
}

enum FocusPane {
    Playlist,
    History,
    Browser,
}

struct App {
    audio: AudioEngine,
    playlist: Playlist,
    browser: FileBrowser,
    config: Config,
    volume: f32,
    status: String,
    is_playing: bool,
    show_browser: bool,
    show_info: bool,
    playlist_state: ListState,
    browser_state: ListState,
    history_state: ListState,
    modal: Modal,
    focus: FocusPane,
    history: Vec<String>,
    is_muted: bool,
    volume_before_mute: f32,
    last_prev_press: Option<std::time::Instant>,
    current_track_start: Option<std::time::Instant>,
    current_track_path: Option<String>,
    help_scroll: u16,
    save_path_input: String,
}

impl App {
    fn new() -> Result<Self, String> {
        let config = Config::load();
        Ok(Self {
            audio: AudioEngine::new()?,
            playlist: Playlist::new(),
            browser: FileBrowser::new(),
            config,
            volume: 1.0,
            status: "Ready".to_string(),
            is_playing: false,
            show_browser: false,
            show_info: false,
            playlist_state: ListState::default(),
            browser_state: ListState::default(),
            history_state: ListState::default(),
            modal: Modal::None,
            focus: FocusPane::Playlist,
            history: Vec::new(),
            is_muted: false,
            volume_before_mute: 1.0,
            last_prev_press: None,
            current_track_start: None,
            current_track_path: None,
            help_scroll: 0,
            save_path_input: String::new(),
        })
    }

    fn add_to_history_if_played_enough(&mut self) {
        if let (Some(start), Some(ref path)) = (self.current_track_start, &self.current_track_path) {
            let elapsed = start.elapsed().as_secs();
            if elapsed >= 15 && (self.history.is_empty() || self.history[0] != *path) {
                self.history.insert(0, path.clone());
                if self.history.len() > 50 {
                    self.history.truncate(50);
                }
            }
        }
    }

    fn play_current(&mut self) {
        // Add previous track to history if it was played long enough
        self.add_to_history_if_played_enough();
        
        if let Some(track) = self.playlist.current() {
            self.audio.stop();
            match self.audio.play(track) {
                Ok(_) => {
                    self.status = format!("Playing: {}", Self::get_filename(track));
                    self.is_playing = true;
                    // Track when this song started
                    self.current_track_start = Some(std::time::Instant::now());
                    self.current_track_path = Some(track.to_string());
                }
                Err(e) => self.status = format!("Error: {}", e),
            }
        }
    }

    fn get_filename(path: &str) -> &str {
        path.split('/').last().unwrap_or(path)
    }

    fn get_metadata(path: &str) -> (String, String, String, String) {
        if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
            let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
            if let Some(tag) = tag {
                let artist = tag.artist().as_deref().unwrap_or("Unknown Artist").to_string();
                let album = tag.album().as_deref().unwrap_or("Unknown Album").to_string();
                let title = tag.title().as_deref().unwrap_or(Self::get_filename(path)).to_string();
                let year = tag.year().map(|y| y.to_string()).unwrap_or_else(|| "Unknown".to_string());
                return (title, artist, album, year);
            }
        }
        (Self::get_filename(path).to_string(), "Unknown Artist".to_string(), "Unknown Album".to_string(), "Unknown".to_string())
    }

    fn format_duration(secs: u64) -> String {
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }

    fn save_playlist_m3u(&self, path: &str) -> Result<(), String> {
        let tracks = self.playlist.tracks();
        if tracks.is_empty() {
            return Err("Playlist is empty".to_string());
        }
        
        let mut content = String::from("#EXTM3U\n");
        for track in tracks {
            content.push_str(track);
            content.push('\n');
        }
        
        std::fs::write(path, content).map_err(|e| format!("Failed to save: {}", e))
    }

    fn get_default_playlist_path(&self) -> String {
        let base_dir = self.config.default_playlist_dir.clone()
            .or_else(|| dirs::home_dir().map(|p| p.join("Music").to_string_lossy().to_string()))
            .unwrap_or_else(|| ".".to_string());
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        format!("{}/playlist_{}.m3u", base_dir, timestamp)
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new()?;
    
    // Load example playlist if provided as argument
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        if let Err(e) = app.playlist.load_m3u(&args[1]) {
            eprintln!("Failed to load playlist: {}", e);
        } else {
            app.config.last_playlist = Some(args[1].clone());
        }
    } else if !app.config.current_playlist_tracks.is_empty() {
        // Restore last session's playlist
        app.playlist.add_tracks(app.config.current_playlist_tracks.clone());
    }
    
    // Set browser to last directory or default music dir
    if let Some(ref last_dir) = app.config.last_directory {
        app.browser = FileBrowser::from_path(last_dir);
    } else if let Some(ref music_dir) = app.config.default_music_dir {
        app.browser = FileBrowser::from_path(music_dir);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut last_progress_update = std::time::Instant::now();
    let mut needs_redraw = true;
    let mut scan_receiver: Option<Receiver<std::path::PathBuf>> = None;
    let mut scan_count = 0;

    loop {
        // Check if track finished and auto-play next
        if app.is_playing && app.audio.is_finished() && app.playlist.tracks().len() > 0 {
            app.playlist.next();
            app.play_current();
            needs_redraw = true;
        }

        // Check for incoming scanned files
        if let Some(ref receiver) = scan_receiver {
            let mut batch = Vec::new();
            while let Ok(path) = receiver.try_recv() {
                batch.push(path.to_string_lossy().to_string());
                scan_count += 1;
                if batch.len() >= 50 {
                    break; // Process in batches
                }
            }
            
            if !batch.is_empty() {
                app.playlist.add_tracks(batch);
                app.status = format!("⟳ Scanning... (added {} files)", scan_count);
                needs_redraw = true;
            }
        }

        // Update progress bar once per second
        if app.is_playing && last_progress_update.elapsed() >= std::time::Duration::from_secs(1) {
            last_progress_update = std::time::Instant::now();
            needs_redraw = true;
        }

        if needs_redraw {
            terminal.draw(|f| {
                let main_chunks = if app.show_browser {
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Percentage(35),  // Browser
                            Constraint::Percentage(65),  // Rest
                        ])
                        .split(f.size());
                    vec![chunks[0], chunks[1]]
                } else {
                    vec![f.size(), f.size()]
                };

                // File Browser (if visible)
                if app.show_browser {
                    let browser_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(3),  // Current dir
                            Constraint::Min(5),     // File list
                        ])
                        .split(main_chunks[0]);

                    // Current directory
                    let dir_display = app.browser.current_dir().to_string_lossy().to_string();
                    let dir_widget = Paragraph::new(dir_display)
                        .style(Style::default().fg(Color::Cyan))
                        .block(Block::default().borders(Borders::ALL).title("Directory"));
                    f.render_widget(dir_widget, browser_chunks[0]);

                    // File list
                    let file_items: Vec<ListItem> = app.browser.entries()
                        .iter()
                        .enumerate()
                        .map(|(i, entry)| {
                            let icon = if entry.is_dir {
                                "▸ "
                            } else if entry.is_playlist {
                                "≡ "
                            } else {
                                "♪ "
                            };
                            
                            let style = if i == app.browser.selected_index() {
                                Style::default().bg(Color::DarkGray)
                            } else {
                                Style::default()
                            };
                            
                            ListItem::new(format!("{}{}", icon, entry.name)).style(style)
                        })
                        .collect();
                    
                    app.browser_state.select(Some(app.browser.selected_index()));
                    
                    let file_list = List::new(file_items)
                        .block(Block::default().borders(Borders::ALL).title("Files [Enter: Add | Backspace: Up | A: Add All]"))
                        .highlight_style(Style::default().bg(Color::DarkGray));
                    f.render_stateful_widget(file_list, browser_chunks[1], &mut app.browser_state);
                }

                // Right side - split into playlist and player controls
                let content_area = if app.show_browser { main_chunks[1] } else { main_chunks[0] };
                
                let main_layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(5),      // Top area (playlist + history)
                        Constraint::Length(5),   // Player at bottom (minimal)
                    ])
                    .split(content_area);

                // Top area: Playlist and History side by side
                let top_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(70),  // Playlist
                        Constraint::Percentage(30),  // History + Controls
                    ])
                    .split(main_layout[0]);

                // Left: Playlist with menu
                let playlist_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),  // Menu
                        Constraint::Min(5),     // Playlist
                    ])
                    .split(top_layout[0]);

                // Menu bar
                let menu = Paragraph::new("RustPlayer | Tab: Browser | F1: Help | F2: Settings | Q: Quit")
                    .style(Style::default().fg(Color::Cyan))
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(menu, playlist_chunks[0]);

                // Playlist
                let items: Vec<ListItem> = app.playlist.tracks()
                    .iter()
                    .enumerate()
                    .map(|(i, track)| {
                        let filename = App::get_filename(track);
                        let mut style = Style::default();
                        
                        if i == app.playlist.current_index() {
                            style = style.fg(Color::Yellow).add_modifier(Modifier::BOLD);
                        }
                        if matches!(app.focus, FocusPane::Playlist) && i == app.playlist.selected_index() {
                            style = style.bg(Color::DarkGray);
                        }
                        
                        let prefix = if i == app.playlist.current_index() { "▶ " } else { "  " };
                        ListItem::new(format!("{}{}", prefix, filename)).style(style)
                    })
                    .collect();
                
                if matches!(app.focus, FocusPane::Playlist) {
                    app.playlist_state.select(Some(app.playlist.selected_index()));
                }
                
                let playlist_title = if matches!(app.focus, FocusPane::Playlist) {
                    "Playlist [Tab: Next]"
                } else {
                    "Playlist"
                };
                
                let playlist_style = if matches!(app.focus, FocusPane::Playlist) {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                
                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title(playlist_title).border_style(playlist_style))
                    .highlight_style(Style::default().bg(Color::DarkGray));
                f.render_stateful_widget(list, playlist_chunks[1], &mut app.playlist_state);

                // Right: History and Controls
                let right_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(5),      // History
                        Constraint::Length(11),  // Keybinds
                    ])
                    .split(top_layout[1]);

                // History
                let history_items: Vec<ListItem> = app.history
                    .iter()
                    .map(|track| {
                        let filename = App::get_filename(track);
                        ListItem::new(format!("♪ {}", filename))
                    })
                    .collect();
                
                if matches!(app.focus, FocusPane::History) && !app.history.is_empty() {
                    if app.history_state.selected().is_none() {
                        app.history_state.select(Some(0));
                    }
                }
                
                let history_title = if matches!(app.focus, FocusPane::History) {
                    "History [H: Focus | Tab: Next | ↑/↓: Scroll]"
                } else {
                    "History [H: Focus]"
                };
                
                let history_style = if matches!(app.focus, FocusPane::History) {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                
                let history_list = List::new(history_items)
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .title(history_title)
                        .border_style(history_style))
                    .highlight_style(Style::default().bg(Color::DarkGray));
                f.render_stateful_widget(history_list, right_chunks[0], &mut app.history_state);

                // Keybinds or Info box
                let info_widget = if app.show_info {
                    // Show track info
                    let info_text = if let Some(track_path) = app.playlist.current() {
                        let (title, artist, album, year) = App::get_metadata(track_path);
                        format!("Title:  {}\nArtist: {}\nAlbum:  {}\nYear:   {}", title, artist, album, year)
                    } else {
                        "No track playing".to_string()
                    };
                    
                    Paragraph::new(info_text)
                        .style(Style::default().fg(Color::Cyan))
                        .block(Block::default().borders(Borders::ALL).title("Track Info [I: Toggle]"))
                } else {
                    // Show keybinds
                    let keybinds_text = 
                        "Space   Play/Pause\n\
                         , .     Prev/Next\n\
                         ← →     Seek ±5s\n\
                         + -     Volume\n\
                         M       Mute\n\
                         S       Shuffle\n\
                         R       Repeat";
                    
                    Paragraph::new(keybinds_text)
                        .style(Style::default().fg(Color::Gray))
                        .block(Block::default().borders(Borders::ALL).title("Controls [I: Info]"))
                };
                f.render_widget(info_widget, right_chunks[1]);

                // Player at bottom (full width)
                let current_track = app.playlist.current()
                    .map(|t| App::get_filename(t))
                    .unwrap_or("No track");
                
                let position = app.audio.get_position();
                let duration = app.audio.get_duration();
                
                let (progress_ratio, time_label) = if let Some(dur) = duration {
                    let pos_secs = position.as_secs();
                    let dur_secs = dur.as_secs();
                    let ratio = if dur_secs > 0 {
                        (pos_secs as f64 / dur_secs as f64).min(1.0)
                    } else {
                        0.0
                    };
                    (ratio, format!("{} / {}", 
                        App::format_duration(pos_secs), 
                        App::format_duration(dur_secs)))
                } else {
                    (0.0, "-- / --".to_string())
                };

                // Build progress bar
                let progress_width = main_layout[1].width.saturating_sub(4) as usize;
                let filled = (progress_width as f64 * progress_ratio) as usize;
                let progress_bar = format!("{}{}",
                    "━".repeat(filled),
                    "─".repeat(progress_width.saturating_sub(filled))
                );

                // Control buttons with state
                let play_btn = if app.is_playing && !app.audio.is_paused() {
                    "▶"  // Show playing status
                } else {
                    "⏸"  // Show paused status
                };
                
                let shuffle_text = "Shuffle";
                let shuffle_style = if app.playlist.is_shuffle() {
                    Style::default().fg(Color::Rgb(255, 165, 0)) // Orange
                } else {
                    Style::default().fg(Color::Gray)
                };
                
                let repeat_text = match app.playlist.repeat_mode() {
                    RepeatMode::Off => "Repeat",
                    RepeatMode::One => "Repeat 1",
                    RepeatMode::All => "Repeat All",
                };
                let repeat_style = match app.playlist.repeat_mode() {
                    RepeatMode::Off => Style::default().fg(Color::Gray),
                    _ => Style::default().fg(Color::Rgb(255, 165, 0)), // Orange
                };

                let vol_display = if app.is_muted {
                    "Vol: MUTED"
                } else {
                    &format!("Vol: {}%", (app.volume * 100.0) as u32)
                };

                // Build player display with styled components
                let player_lines = vec![
                    Line::from(vec![Span::styled(format!("♪ {} | {}", current_track, time_label), Style::default())]),
                    Line::from(progress_bar.clone()),
                    Line::from(vec![
                        Span::raw("  "),
                        Span::raw(play_btn),
                        Span::raw("  "),
                        Span::styled(shuffle_text, shuffle_style),
                        Span::raw("  "),
                        Span::styled(repeat_text, repeat_style),
                        Span::raw(format!("  {}", vol_display)),
                    ]),
                ];
                
                let player = Paragraph::new(player_lines)
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL).title("Player"));
                f.render_widget(player, main_layout[1]);
                
                // Render modals on top
                match app.modal {
                    Modal::Help => {
                        let area = centered_rect(60, 70, f.size());
                        f.render_widget(Clear, area);
                        let help_text = vec![
                            "RustPlayer - Help",
                            "",
                            "Global Controls:",
                            "  Space     - Play/Pause",
                            "  , / .     - Previous/Next track",
                            "  ← / →     - Seek ±5 seconds",
                            "  + / -     - Volume up/down",
                            "  M         - Mute/Unmute",
                            "  Tab       - Toggle file browser",
                            "  H         - Toggle history",
                            "  I         - Toggle track info",
                            "  F1        - Show this help",
                            "  F2        - Settings",
                            "  Q         - Quit",
                            "",
                            "Playlist:",
                            "  ↑ / ↓     - Navigate playlist",
                            "  Enter     - Play selected track",
                            "  Delete    - Remove selected track",
                            "  C         - Clear entire playlist",
                            "  S         - Toggle shuffle",
                            "  R         - Cycle repeat mode",
                            "  Ctrl+S    - Save playlist as M3U",
                            "",
                            "File Browser (when visible):",
                            "  ↑ / ↓     - Navigate files",
                            "  Enter     - Enter folder / Add file",
                            "  Backspace - Go up one directory",
                            "  A         - Add all audio in folder",
                            "  Ctrl+D    - Set as default music dir",
                            "",
                            "Press ESC or F1 to close",
                        ];
                        let help = Paragraph::new(help_text.join("\n"))
                            .block(Block::default().borders(Borders::ALL).title("Help [↑/↓ to scroll]"))
                            .style(Style::default().bg(Color::Black))
                            .scroll((app.help_scroll, 0))
                            .wrap(Wrap { trim: false });
                        f.render_widget(help, area);
                    }
                    Modal::Settings => {
                        let area = centered_rect(60, 50, f.size());
                        f.render_widget(Clear, area);
                        
                        let default_dir = app.config.default_music_dir.as_deref().unwrap_or("Not set");
                        let playlist_dir = app.config.default_playlist_dir.as_deref().unwrap_or("~/Music (default)");
                        let last_dir = app.config.last_directory.as_deref().unwrap_or("Not set");
                        
                        let settings_text = format!(
                            "RustPlayer - Settings\n\n\
                            Default Music Directory:\n  {}\n\n\
                            Default Playlist Save Directory:\n  {}\n\n\
                            Last Directory:\n  {}\n\n\
                            Note: Settings are automatically saved.\n\
                            To set default music dir, navigate to it\n\
                            in the browser and press Ctrl+D.\n\n\
                            Press ESC or F2 to close",
                            default_dir, playlist_dir, last_dir
                        );
                        
                        let settings = Paragraph::new(settings_text)
                            .block(Block::default().borders(Borders::ALL).title("Settings"))
                            .style(Style::default().bg(Color::Black))
                            .wrap(Wrap { trim: false });
                        f.render_widget(settings, area);
                    }
                    Modal::SavePlaylist => {
                        let area = centered_rect(70, 30, f.size());
                        f.render_widget(Clear, area);
                        
                        let save_text = format!(
                            "Save Playlist\n\n\
                            Path:\n{}\n\n\
                            Press Enter to save, ESC to cancel\n\
                            Use Backspace to edit path",
                            app.save_path_input
                        );
                        
                        let save_dialog = Paragraph::new(save_text)
                            .block(Block::default().borders(Borders::ALL).title("Save Playlist as M3U"))
                            .style(Style::default().bg(Color::Black))
                            .wrap(Wrap { trim: false });
                        f.render_widget(save_dialog, area);
                    }
                    Modal::None => {}
                }
            })?;
            needs_redraw = false;
        }

        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                needs_redraw = true;
                
                // Modal handling
                match app.modal {
                    Modal::SavePlaylist => {
                        match key.code {
                            KeyCode::Esc => {
                                app.modal = Modal::None;
                                app.save_path_input.clear();
                            }
                            KeyCode::Enter => {
                                let path = app.save_path_input.clone();
                                match app.save_playlist_m3u(&path) {
                                    Ok(_) => app.status = format!("Playlist saved: {}", path),
                                    Err(e) => app.status = format!("Error: {}", e),
                                }
                                app.modal = Modal::None;
                                app.save_path_input.clear();
                            }
                            KeyCode::Backspace => {
                                app.save_path_input.pop();
                            }
                            KeyCode::Char(c) => {
                                app.save_path_input.push(c);
                            }
                            _ => {}
                        }
                        continue;
                    }
                    Modal::Help | Modal::Settings => {
                        match key.code {
                            KeyCode::Esc | KeyCode::F(1) if matches!(app.modal, Modal::Help) => {
                                app.modal = Modal::None;
                                app.help_scroll = 0;
                            }
                            KeyCode::Esc | KeyCode::F(2) if matches!(app.modal, Modal::Settings) => {
                                app.modal = Modal::None;
                            }
                            KeyCode::Up if matches!(app.modal, Modal::Help) => {
                                app.help_scroll = app.help_scroll.saturating_sub(1);
                            }
                            KeyCode::Down if matches!(app.modal, Modal::Help) => {
                                app.help_scroll = app.help_scroll.saturating_add(1);
                            }
                            _ => {}
                        }
                        continue;
                    }
                    Modal::None => {}
                }
                
                // Global keys
                match key.code {
                    KeyCode::Char('q') => {
                        // Save config before quitting
                        app.config.last_directory = Some(app.browser.current_dir().to_string_lossy().to_string());
                        app.config.current_playlist_tracks = app.playlist.tracks().to_vec();
                        app.config.save();
                        break;
                    }
                    KeyCode::F(1) => {
                        app.modal = Modal::Help;
                    }
                    KeyCode::F(2) => {
                        app.modal = Modal::Settings;
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        // Toggle between Playlist and History
                        app.focus = match app.focus {
                            FocusPane::History => FocusPane::Playlist,
                            _ => FocusPane::History,
                        };
                    }
                    KeyCode::Char('i') | KeyCode::Char('I') => {
                        // Toggle info view
                        app.show_info = !app.show_info;
                    }
                    KeyCode::Tab => {
                        // Tab toggles browser and switches focus
                        if app.show_browser {
                            // Browser is open, close it and go to playlist
                            app.show_browser = false;
                            app.focus = FocusPane::Playlist;
                        } else {
                            // Browser is closed, open it and focus it
                            app.show_browser = true;
                            app.focus = FocusPane::Browser;
                            app.config.last_directory = Some(app.browser.current_dir().to_string_lossy().to_string());
                        }
                    }
                    KeyCode::Char(' ') => {
                        if app.audio.is_paused() {
                            app.audio.resume();
                            app.is_playing = true;
                        } else {
                            app.audio.pause();
                            app.is_playing = false;
                        }
                    }
                    KeyCode::Left => {
                        app.audio.seek_backward(5);
                    }
                    KeyCode::Right => {
                        app.audio.seek_forward(5);
                    }
                    KeyCode::Char(',') => {
                        // If pressed within 2 seconds of last press, go to previous track
                        // Otherwise, restart current track
                        let now = std::time::Instant::now();
                        let should_go_prev = if let Some(last) = app.last_prev_press {
                            now.duration_since(last) < std::time::Duration::from_secs(2)
                        } else {
                            false
                        };
                        
                        if should_go_prev || app.audio.get_position().as_secs() < 3 {
                            // Go to previous track
                            app.playlist.previous();
                            app.play_current();
                            app.last_prev_press = None;
                        } else {
                            // Restart current track
                            app.play_current();
                            app.last_prev_press = Some(now);
                        }
                    }
                    KeyCode::Char('.') => {
                        app.playlist.next();
                        app.play_current();
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        if !app.is_muted {
                            app.volume = (app.volume + 0.1).min(2.0);
                            app.audio.set_volume(app.volume);
                        }
                    }
                    KeyCode::Char('-') => {
                        if !app.is_muted {
                            app.volume = (app.volume - 0.1).max(0.0);
                            app.audio.set_volume(app.volume);
                        }
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') => {
                        if app.is_muted {
                            app.is_muted = false;
                            app.volume = app.volume_before_mute;
                            app.audio.set_volume(app.volume);
                        } else {
                            app.is_muted = true;
                            app.volume_before_mute = app.volume;
                            app.audio.set_volume(0.0);
                        }
                    }
                    KeyCode::Char('s') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        // Open save playlist modal
                        app.save_path_input = app.get_default_playlist_path();
                        app.modal = Modal::SavePlaylist;
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        app.playlist.toggle_shuffle();
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        app.playlist.cycle_repeat();
                    }
                    _ => {
                        // Context-specific keys based on focus
                        match app.focus {
                            FocusPane::Browser if app.show_browser => {
                                match key.code {
                                    KeyCode::Up => app.browser.select_prev(),
                                    KeyCode::Down => app.browser.select_next(),
                                    KeyCode::Backspace => {
                                        app.browser.go_up();
                                        app.config.last_directory = Some(app.browser.current_dir().to_string_lossy().to_string());
                                    }
                                    KeyCode::Char('d') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                                        app.config.default_music_dir = Some(app.browser.current_dir().to_string_lossy().to_string());
                                        app.config.save();
                                        app.status = "Default music directory set".to_string();
                                    }
                                    KeyCode::Enter => {
                                        if let Some(entry) = app.browser.enter_selected() {
                                            if entry.is_playlist {
                                                if let Err(e) = app.playlist.load_m3u(&entry.path.to_string_lossy()) {
                                                    app.status = format!("Error loading playlist: {}", e);
                                                } else {
                                                    app.status = format!("Loaded playlist: {}", entry.name);
                                                    app.config.last_playlist = Some(entry.path.to_string_lossy().to_string());
                                                    app.config.save();
                                                }
                                            } else if entry.is_audio {
                                                app.playlist.add_track(entry.path.to_string_lossy().to_string());
                                                app.status = format!("Added: {}", entry.name);
                                            }
                                        } else {
                                            app.config.last_directory = Some(app.browser.current_dir().to_string_lossy().to_string());
                                        }
                                    }
                                    KeyCode::Char('a') | KeyCode::Char('A') => {
                                        let scan_dir = app.browser.current_dir().to_path_buf();
                                        let (sender, receiver) = channel();
                                        scan_receiver = Some(receiver);
                                        scan_count = 0;
                                        
                                        app.status = "⟳ Starting scan...".to_string();
                                        
                                        thread::spawn(move || {
                                            FileBrowser::scan_audio_files_streaming(scan_dir, sender);
                                        });
                                    }
                                    _ => { needs_redraw = false; }
                                }
                            }
                            FocusPane::History => {
                                match key.code {
                                    KeyCode::Up => {
                                        let len = app.history.len();
                                        if len > 0 {
                                            let current = app.history_state.selected().unwrap_or(0);
                                            let next = if current == 0 { len - 1 } else { current - 1 };
                                            app.history_state.select(Some(next));
                                        }
                                    }
                                    KeyCode::Down => {
                                        let len = app.history.len();
                                        if len > 0 {
                                            let current = app.history_state.selected().unwrap_or(0);
                                            let next = (current + 1) % len;
                                            app.history_state.select(Some(next));
                                        }
                                    }
                                    KeyCode::Enter => {
                                        // Play song from history
                                        if let Some(selected) = app.history_state.selected() {
                                            if let Some(track) = app.history.get(selected) {
                                                let track_path = track.clone();
                                                
                                                // Check if track is in playlist
                                                if let Some(pos) = app.playlist.tracks().iter().position(|t| t == &track_path) {
                                                    // Track exists, jump to it
                                                    app.playlist.select_index(pos);
                                                    app.playlist.play_selected();
                                                } else {
                                                    // Track not in playlist, add it and play
                                                    app.playlist.add_track(track_path.clone());
                                                    app.playlist.select_index(app.playlist.tracks().len() - 1);
                                                    app.playlist.play_selected();
                                                }
                                                app.play_current();
                                            }
                                        }
                                    }
                                    _ => { needs_redraw = false; }
                                }
                            }
                            FocusPane::Playlist => {
                                match key.code {
                                    KeyCode::Up => app.playlist.select_prev(),
                                    KeyCode::Down => app.playlist.select_next(),
                                    KeyCode::Enter => {
                                        app.playlist.play_selected();
                                        app.play_current();
                                    }
                                    KeyCode::Delete => {
                                        if app.playlist.remove_selected() {
                                            app.status = "Track removed".to_string();
                                        }
                                    }
                                    KeyCode::Char('c') | KeyCode::Char('C') => {
                                        app.playlist.clear();
                                        app.audio.stop();
                                        app.is_playing = false;
                                        app.status = "Playlist cleared".to_string();
                                    }
                                    _ => { needs_redraw = false; }
                                }
                            }
                            _ => { needs_redraw = false; }
                        }
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
