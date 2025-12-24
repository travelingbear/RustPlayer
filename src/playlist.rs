use std::fs;
use std::path::Path;
use rand::seq::SliceRandom;

#[derive(Clone, Copy, PartialEq)]
pub enum RepeatMode {
    Off,
    One,
    All,
}

pub struct Playlist {
    tracks: Vec<String>,
    current: usize,
    selected: usize,
    shuffle: bool,
    repeat: RepeatMode,
}

impl Playlist {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            current: 0,
            selected: 0,
            shuffle: false,
            repeat: RepeatMode::Off,
        }
    }

    pub fn load_m3u(&mut self, path: &str) -> Result<(), String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read M3U: {}", e))?;
        
        let base_dir = Path::new(path).parent().unwrap_or(Path::new("."));
        
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                let track_path = base_dir.join(line);
                self.tracks.push(track_path.to_string_lossy().to_string());
            }
        }
        Ok(())
    }

    pub fn current(&self) -> Option<&str> {
        self.tracks.get(self.current).map(|s| s.as_str())
    }

    pub fn next(&mut self) -> Option<&str> {
        if self.tracks.is_empty() {
            return None;
        }
        
        match self.repeat {
            RepeatMode::One => {},
            RepeatMode::All => {
                self.current = (self.current + 1) % self.tracks.len();
            }
            RepeatMode::Off => {
                if self.current + 1 < self.tracks.len() {
                    self.current += 1;
                }
            }
        }
        self.selected = self.current;
        self.current()
    }

    pub fn previous(&mut self) -> Option<&str> {
        if self.tracks.is_empty() {
            return None;
        }
        
        self.current = if self.current == 0 {
            self.tracks.len() - 1
        } else {
            self.current - 1
        };
        self.selected = self.current;
        self.current()
    }

    pub fn select_next(&mut self) {
        if !self.tracks.is_empty() {
            self.selected = (self.selected + 1) % self.tracks.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.tracks.is_empty() {
            self.selected = if self.selected == 0 {
                self.tracks.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn play_selected(&mut self) {
        self.current = self.selected;
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
        if self.shuffle && !self.tracks.is_empty() {
            let current_track = self.tracks[self.current].clone();
            let mut rng = rand::thread_rng();
            self.tracks.shuffle(&mut rng);
            // Keep current track at current position
            if let Some(pos) = self.tracks.iter().position(|t| t == &current_track) {
                self.tracks.swap(self.current, pos);
            }
        }
    }

    pub fn cycle_repeat(&mut self) {
        self.repeat = match self.repeat {
            RepeatMode::Off => RepeatMode::One,
            RepeatMode::One => RepeatMode::All,
            RepeatMode::All => RepeatMode::Off,
        };
    }

    pub fn tracks(&self) -> &[String] {
        &self.tracks
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn is_shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat
    }

    pub fn add_track(&mut self, path: String) {
        self.tracks.push(path);
        if self.tracks.len() == 1 {
            self.selected = 0;
            self.current = 0;
        }
    }

    pub fn add_tracks(&mut self, paths: Vec<String>) {
        let was_empty = self.tracks.is_empty();
        for path in paths {
            self.tracks.push(path);
        }
        if was_empty && !self.tracks.is_empty() {
            self.selected = 0;
            self.current = 0;
        }
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
        self.current = 0;
        self.selected = 0;
    }

    pub fn remove_selected(&mut self) -> bool {
        if self.selected < self.tracks.len() {
            self.tracks.remove(self.selected);
            
            // Adjust indices
            if self.tracks.is_empty() {
                self.current = 0;
                self.selected = 0;
            } else {
                if self.selected >= self.tracks.len() {
                    self.selected = self.tracks.len() - 1;
                }
                if self.current >= self.tracks.len() {
                    self.current = self.tracks.len() - 1;
                }
            }
            true
        } else {
            false
        }
    }
}
