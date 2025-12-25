use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub last_directory: Option<String>,
    pub last_playlist: Option<String>,
    pub default_music_dir: Option<String>,
    pub default_playlist_dir: Option<String>,
    pub current_playlist_tracks: Vec<String>,
}

impl Config {
    pub fn load() -> Self {
        let config_path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&config_path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        if let Ok(content) = serde_json::to_string_pretty(self) {
            if let Some(parent) = Self::config_path().parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::write(Self::config_path(), content).ok();
        }
    }

    fn config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("rustplayer");
        path.push("config.json");
        path
    }
}
