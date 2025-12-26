# TAP - Terminal Audio Player

A lightweight, terminal-based music player written in Rust with minimal resource usage.

Part of the **TipTap** suite (TAP + TYP - Terminal YouTube Player).

## Features

- **Audio Playback**: Supports MP3, FLAC, WAV, and OGG formats
- **Playlist Management**: Add, remove, shuffle, and repeat tracks
- **File Browser**: Navigate and add music from your filesystem
- **Playback History**: Tracks your last 50 played songs (only songs played for 15+ seconds)
- **Playback Controls**: Play, pause, seek, volume control, and mute
- **Smart Previous**: Restarts track if >3 seconds in, goes to previous if <3 seconds
- **Track Metadata**: Display artist, album, title, and year (toggle with 'I')
- **Playlist Save/Load**: Save playlists as M3U files (Ctrl+S)
- **Desktop Integration**: Open audio files from file manager
- **Minimal Resource Usage**: ~11-13 MB RAM, ~2-3% CPU during playback

## Installation

### From Binary

Download the latest release for your platform from the [Releases](../../releases) page.

**Linux:**
```bash
wget https://github.com/travelingbear/RustPlayer/releases/latest/download/tap-linux-x86_64
chmod +x tap-linux-x86_64
sudo mv tap-linux-x86_64 /usr/local/bin/tap
```

**macOS:**
```bash
# Intel
wget https://github.com/travelingbear/RustPlayer/releases/latest/download/tap-macos-x86_64
# Apple Silicon
wget https://github.com/travelingbear/RustPlayer/releases/latest/download/tap-macos-aarch64

chmod +x tap-macos-*
sudo mv tap-macos-* /usr/local/bin/tap
```

### From Source

Requires Rust 1.70 or later.

```bash
git clone https://github.com/travelingbear/RustPlayer.git
cd RustPlayer
cargo build --release
./target/release/tap
```

### Desktop Integration (Linux)

To open audio files from your file manager:

1. Install the wrapper script:
```bash
sudo cp tap-open /usr/local/bin/
```

2. Install the desktop file:
```bash
cp tap.desktop ~/.local/share/applications/
update-desktop-database ~/.local/share/applications/
```

Now you can right-click audio files and select "Open With TAP Audio Player".

## Usage

### Keybindings

**Playback Controls:**
- `Space` - Play/Pause
- `,` - Previous track (restart if >3s into song)
- `.` - Next track
- `←` `→` - Seek backward/forward 5 seconds
- `+` `=` - Increase volume
- `-` - Decrease volume
- `M` - Mute/Unmute

**Playlist Controls:**
- `S` - Toggle shuffle (Off/On)
- `R` - Cycle repeat mode (Off/One/All)
- `Delete` - Remove selected track
- `C` - Clear playlist (works globally, even with modals open)
- `Ctrl+S` - Save playlist as M3U

**Navigation:**
- `Tab` - Toggle file browser
- `↑` `↓` - Navigate lists
- `Enter` - Play selected track or add directory
- `Backspace` - Go up directory (in browser) or open browser (in playlist)
- `H` - Toggle history view
- `I` - Toggle track info display
- `F1` - Help
- `F2` - Settings
- `Q` - Quit

## Performance

TAP is designed to be extremely resource-efficient:
- **Memory**: ~11-13 MB
- **CPU**: ~2-3% during playback
- No memory leaks during extended playback sessions

## Technical Details

- Built with [rodio](https://github.com/RustAudio/rodio) for audio playback
- Uses [symphonia](https://github.com/pdeljanov/Symphonia) for accurate duration detection
- Terminal UI powered by [ratatui](https://github.com/ratatui-org/ratatui)
- Metadata reading with [lofty](https://github.com/Serial-ATA/lofty-rs)
- Persistent configuration with TOML

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history.

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.
