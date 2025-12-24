# RustPlayer

A minimal, battery-efficient terminal-based audio player.

## Build

```bash
cargo build --release
```

## Usage

```bash
# With M3U playlist
cargo run example.m3u

# Or run the binary
./target/release/rustplayer example.m3u
```

## Controls

- **Space**: Play/Pause
- **N**: Next track
- **P**: Previous track
- **+/-**: Volume up/down
- **Enter**: Play current track
- **Q**: Quit

## Supported Formats

MP3, FLAC, WAV, OGG, and other formats supported by rodio.

## M3U Playlist Format

Create a `.m3u` file with one audio file path per line:

```
/path/to/song1.mp3
/path/to/song2.flac
relative/path/song3.wav
```
