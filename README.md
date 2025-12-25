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
- **Minimal Resource Usage**: ~11.6 MB RAM, ~4% CPU during playback

## Installation

### From Source

Requires Rust 1.70 or later.

```bash
git clone <your-repo-url>
cd TAP
cargo build --release
./target/release/rustplayer
```

### From Binary

Download the latest release for your platform from the [Releases](../../releases) page.

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
- `C` - Clear playlist

**Navigation:**
- `Tab` - Cycle between panes (Playlist/Browser)
- `↑` `↓` - Navigate lists
- `Enter` - Play selected track (in Playlist/History) or add directory (in Browser)
- `H` - Toggle history view
- `B` - Toggle file browser
- `Q` - Quit

## Performance

TAP is designed to be extremely resource-efficient:
- **Memory**: ~11.6 MB
- **CPU**: ~4% during playback
- No memory leaks during extended playback sessions

## Technical Details

- Built with [rodio](https://github.com/RustAudio/rodio) for audio playback
- Uses [symphonia](https://github.com/pdeljanov/Symphonia) for accurate duration detection
- Terminal UI powered by [ratatui](https://github.com/ratatui-org/ratatui)
- Persistent configuration with JSON

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.
