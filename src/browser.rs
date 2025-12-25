use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};

#[derive(Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_audio: bool,
    pub is_playlist: bool,
}

pub struct FileBrowser {
    current_dir: PathBuf,
    entries: Vec<FileEntry>,
    selected: usize,
}

impl FileBrowser {
    pub fn new() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut browser = Self {
            current_dir: current_dir.clone(),
            entries: Vec::new(),
            selected: 0,
        };
        browser.load_directory();
        browser
    }

    pub fn from_path(path: &str) -> Self {
        let current_dir = PathBuf::from(path);
        let current_dir = if current_dir.exists() {
            current_dir
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        
        let mut browser = Self {
            current_dir,
            entries: Vec::new(),
            selected: 0,
        };
        browser.load_directory();
        browser
    }

    fn load_directory(&mut self) {
        self.entries.clear();
        self.selected = 0;

        let Ok(read_dir) = fs::read_dir(&self.current_dir) else {
            return;
        };

        let mut entries: Vec<FileEntry> = read_dir
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                
                // Skip hidden files
                if name.starts_with('.') {
                    return None;
                }

                let is_dir = path.is_dir();
                
                if is_dir {
                    return Some(FileEntry {
                        path,
                        name,
                        is_dir: true,
                        is_audio: false,
                        is_playlist: false,
                    });
                }
                
                // Check file extension
                let extension = path.extension()?.to_str()?.to_lowercase();
                let is_audio = matches!(extension.as_str(), "mp3" | "flac" | "wav" | "ogg");
                let is_playlist = extension == "m3u";

                if is_audio || is_playlist {
                    Some(FileEntry {
                        path,
                        name,
                        is_dir: false,
                        is_audio,
                        is_playlist,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort: directories first, then files alphabetically
        entries.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        self.entries = entries;
    }

    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1) % self.entries.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.entries.is_empty() {
            self.selected = if self.selected == 0 {
                self.entries.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn enter_selected(&mut self) -> Option<FileEntry> {
        let entry = self.entries.get(self.selected)?.clone();
        
        if entry.is_dir {
            self.current_dir = entry.path.clone();
            self.load_directory();
            None
        } else {
            Some(entry)
        }
    }

    pub fn go_up(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.load_directory();
        }
    }

    pub fn scan_audio_files_streaming(dir: PathBuf, sender: Sender<PathBuf>) {
        Self::collect_audio_files_streaming(&dir, 0, &sender, 0);
    }

    fn collect_audio_files_streaming(dir: &Path, depth: usize, sender: &Sender<PathBuf>, file_count: usize) -> usize {
        // Limit recursion depth and total files
        if depth > 8 || file_count > 5000 {
            return file_count;
        }
        
        let mut count = file_count;
        
        let Ok(entries) = fs::read_dir(dir) else {
            return count;
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            
            // Skip hidden files/folders
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
            }
            
            if path.is_dir() {
                count = Self::collect_audio_files_streaming(&path, depth + 1, sender, count);
            } else if let Some(ext) = path.extension() {
                if matches!(ext.to_str(), Some("mp3" | "flac" | "wav" | "ogg")) {
                    if sender.send(path).is_err() {
                        return count; // Channel closed, stop scanning
                    }
                    count += 1;
                }
            }
            
            // Safety limit
            if count >= 5000 {
                return count;
            }
        }
        
        count
    }

    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn current_dir(&self) -> &Path {
        &self.current_dir
    }
}
