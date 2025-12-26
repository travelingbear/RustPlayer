# Changelog

All notable changes to TAP (Terminal Audio Player) will be documented in this file.

## [0.2.0] - 2025-12-26

### Added
- Global 'C' key to clear playlist - now works even when help/settings/save modals are open
- Backspace key in playlist pane opens browser pane for intuitive navigation
- Desktop file integration for opening audio files from file manager
- Wrapper script for file associations

### Fixed
- Playlist no longer cycles when repeat mode is off
- Play/pause icon now correctly shows current state (▶ playing, ⏸ paused)

### Changed
- Improved keyboard navigation between panes

## [0.1.0] - 2024-XX-XX

### Added
- Initial release
- Terminal-based audio player with TUI
- Support for MP3, FLAC, OGG, WAV formats
- Playlist management with shuffle and repeat modes
- File browser for loading music
- Track history (15+ second threshold)
- Track metadata display (artist, album, title, year)
- Playlist save/load (M3U format)
- Keyboard shortcuts for all controls
- Low resource usage (~12 MB RAM, ~2.5% CPU)
