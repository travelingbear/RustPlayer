mod audio;
mod playlist;
mod browser;
mod config;

use audio::AudioEngine;
use playlist::{Playlist, RepeatMode};
use browser::FileBrowser;
use config::Config;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Alignment, Rect},
    style::{Color, Modifier, Style},
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
    playlist_state: ListState,
    browser_state: ListState,
    modal: Modal,
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
            playlist_state: ListState::default(),
            browser_state: ListState::default(),
            modal: Modal::None,
        })
    }

    fn play_current(&mut self) {
        if let Some(track) = self.playlist.current() {
            self.audio.stop();
            match self.audio.play(track) {
                Ok(_) => {
                    self.status = format!("Playing: {}", Self::get_filename(track));
                    self.is_playing = true;
                }
                Err(e) => self.status = format!("Error: {}", e),
            }
        }
    }

    fn get_filename(path: &str) -> &str {
        path.split('/').last().unwrap_or(path)
    }

    fn format_duration(secs: u64) -> String {
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}", mins, secs)
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
                        .block(Block::default().borders(Borders::ALL).title("Files [Tab: Hide | Enter: Add | Backspace: Up | A: Add All]"))
                        .highlight_style(Style::default().bg(Color::DarkGray));
                    f.render_stateful_widget(file_list, browser_chunks[1], &mut app.browser_state);
                }

                // Right side (or full screen if browser hidden)
                let content_area = if app.show_browser { main_chunks[1] } else { main_chunks[0] };
                
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),  // Menu
                        Constraint::Min(5),     // Playlist
                        Constraint::Length(3),  // Progress
                        Constraint::Length(3),  // Status
                    ])
                    .split(content_area);

                // Menu bar
                let menu_text = if app.show_browser {
                    "RustPlayer | Tab: Browser | Space: Play/Pause | ↑/↓: Navigate | F1: Help | Q: Quit"
                } else {
                    "RustPlayer | Tab: Browser | Space: Play/Pause | ↑/↓: Select | Enter: Play | F1: Help | Q: Quit"
                };
                let menu = Paragraph::new(menu_text)
                    .style(Style::default().fg(Color::Cyan))
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(menu, chunks[0]);

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
                        if !app.show_browser && i == app.playlist.selected_index() {
                            style = style.bg(Color::DarkGray);
                        }
                        
                        let prefix = if i == app.playlist.current_index() { "▶ " } else { "  " };
                        ListItem::new(format!("{}{}", prefix, filename)).style(style)
                    })
                    .collect();
                
                if !app.show_browser {
                    app.playlist_state.select(Some(app.playlist.selected_index()));
                }
                
                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title("Playlist"))
                    .highlight_style(Style::default().bg(Color::DarkGray));
                f.render_stateful_widget(list, chunks[1], &mut app.playlist_state);

                // Progress bar
                let position = app.audio.get_position();
                let duration = app.audio.get_duration();
                
                let (progress_ratio, progress_label) = if let Some(dur) = duration {
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

                let progress = Gauge::default()
                    .block(Block::default().borders(Borders::ALL).title("Progress"))
                    .gauge_style(Style::default().fg(Color::Green))
                    .ratio(progress_ratio)
                    .label(progress_label);
                f.render_widget(progress, chunks[2]);

                // Status
                let play_state = if app.is_playing && !app.audio.is_paused() {
                    "▶ Playing"
                } else {
                    "⏸ Paused"
                };
                
                let shuffle_state = if app.playlist.is_shuffle() { "⤨ Shuffle" } else { "" };
                let repeat_state = match app.playlist.repeat_mode() {
                    RepeatMode::Off => "",
                    RepeatMode::One => "↻ Repeat One",
                    RepeatMode::All => "⟲ Repeat All",
                };
                
                let status_text = format!("{} | {} | {} | {} | Vol: {}%", 
                    play_state,
                    shuffle_state,
                    repeat_state,
                    app.status,
                    (app.volume * 100.0) as u32
                );
                let status = Paragraph::new(status_text)
                    .block(Block::default().borders(Borders::ALL).title("Status"));
                f.render_widget(status, chunks[3]);
                
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
                            "  + / -     - Volume up/down",
                            "  Tab       - Toggle file browser",
                            "  F1        - Show this help",
                            "  F2        - Settings",
                            "  Q         - Quit",
                            "",
                            "Playlist (when browser hidden):",
                            "  ↑ / ↓     - Navigate playlist",
                            "  Enter     - Play selected track",
                            "  Delete    - Remove selected track",
                            "  C         - Clear entire playlist",
                            "  S         - Toggle shuffle",
                            "  R         - Cycle repeat mode",
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
                            .block(Block::default().borders(Borders::ALL).title("Help"))
                            .style(Style::default().bg(Color::Black))
                            .wrap(Wrap { trim: false });
                        f.render_widget(help, area);
                    }
                    Modal::Settings => {
                        let area = centered_rect(60, 50, f.size());
                        f.render_widget(Clear, area);
                        
                        let default_dir = app.config.default_music_dir.as_deref().unwrap_or("Not set");
                        let last_dir = app.config.last_directory.as_deref().unwrap_or("Not set");
                        let last_playlist = app.config.last_playlist.as_deref().unwrap_or("Not set");
                        
                        let settings_text = format!(
                            "RustPlayer - Settings\n\n\
                            Default Music Directory:\n  {}\n\n\
                            Last Directory:\n  {}\n\n\
                            Last Playlist:\n  {}\n\n\
                            Note: Settings are automatically saved.\n\
                            To set default music dir, navigate to it\n\
                            in the browser and press Ctrl+D.\n\n\
                            Press ESC or F2 to close",
                            default_dir, last_dir, last_playlist
                        );
                        
                        let settings = Paragraph::new(settings_text)
                            .block(Block::default().borders(Borders::ALL).title("Settings"))
                            .style(Style::default().bg(Color::Black))
                            .wrap(Wrap { trim: false });
                        f.render_widget(settings, area);
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
                    Modal::Help | Modal::Settings => {
                        match key.code {
                            KeyCode::Esc | KeyCode::F(1) if matches!(app.modal, Modal::Help) => {
                                app.modal = Modal::None;
                            }
                            KeyCode::Esc | KeyCode::F(2) if matches!(app.modal, Modal::Settings) => {
                                app.modal = Modal::None;
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
                    KeyCode::Tab => {
                        app.show_browser = !app.show_browser;
                        if app.show_browser {
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
                    KeyCode::Char(',') | KeyCode::Char('<') => {
                        app.playlist.previous();
                        app.play_current();
                    }
                    KeyCode::Char('.') | KeyCode::Char('>') => {
                        app.playlist.next();
                        app.play_current();
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        app.volume = (app.volume + 0.1).min(2.0);
                        app.audio.set_volume(app.volume);
                    }
                    KeyCode::Char('-') => {
                        app.volume = (app.volume - 0.1).max(0.0);
                        app.audio.set_volume(app.volume);
                    }
                    _ => {
                        // Context-specific keys
                        if app.show_browser {
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
                        } else {
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
                                KeyCode::Char('s') | KeyCode::Char('S') => {
                                    app.playlist.toggle_shuffle();
                                }
                                KeyCode::Char('r') | KeyCode::Char('R') => {
                                    app.playlist.cycle_repeat();
                                }
                                _ => { needs_redraw = false; }
                            }
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
