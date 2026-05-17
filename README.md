<pre>
       ███▄    █  ▒█████  ▄████▄  ▄▄▄█████▓ █    ██  ███▄    █ ▓█████
       ██ ▀█   █ ▒██▒  ██▒▒██▀ ▀█  ▓  ██▒ ▓▒ ██  ▓██▒ ██ ▀█   █ ▓█   ▀
      ▓██  ▀█ ██▒▒██░  ██▒▒▓█    ▄ ▒ ▓██░ ▒░▓██  ▒██░▓██  ▀█ ██▒▒███
      ▓██▒  ▐▌██▒▒██   ██░▒▓▓▄ ▄██▒░ ▓██▓ ░ ▓▓█  ░██░▓██▒  ▐▌██▒▒▓█  ▄
      ▒██░   ▓██░░ ████▓▒░▒ ▓███▀ ░  ▒██▒ ░ ▒▒█████▓ ▒██░   ▓██░░▒████▒
</pre>

A fully customizable terminal music player in Rust. ASCII-art TUI, multi-format playback, theme system via TOML.

## Demo

<video src="https://github.com/WendellOttoni/Noctune/releases/download/v0.1.0/Gravacao.de.Tela.2026-05-17.123515.mp4" controls width="100%"></video>

> Status: early MVP — plays local files, queue/library navigation, themable UI.

## Installation

### Windows — install script (recommended)

```powershell
irm https://raw.githubusercontent.com/WendellOttoni/Noctune/main/install.ps1 | iex
```

Downloads the binary and adds it to your PATH. Open a new terminal and run `noctune`.

### Linux / macOS — install script

```sh
curl -fsSL https://raw.githubusercontent.com/WendellOttoni/Noctune/main/install.sh | sh
```

Installs to `~/.local/bin/noctune`.

### Windows — Scoop

```powershell
scoop bucket add noctune https://github.com/WendellOttoni/Noctune
scoop install noctune
```

### Build from source (requires Rust)

```sh
cargo install --git https://github.com/WendellOttoni/Noctune
```

Or clone and build locally:

```sh
git clone https://github.com/WendellOttoni/Noctune
cd Noctune
cargo build --release
./target/release/noctune
```

### Manual download

Pre-compiled binaries for Windows, Linux, and macOS (ARM) are available on the [Releases page](https://github.com/WendellOttoni/Noctune/releases).

---

> **Note:** YouTube streaming requires [yt-dlp](https://github.com/yt-dlp/yt-dlp) installed and available in your PATH.

## Features

- Plays MP3, FLAC, WAV, OGG, Opus, M4A, AAC (via Symphonia + rodio)
- TUI built on ratatui + crossterm
- Library / queue / now-playing panes with focus-aware borders
- Waveform and spectrum visualizer
- Theme files in TOML: colors, symbols, ASCII art logo
- 3-band equalizer with presets
- YouTube / HTTP stream playback via yt-dlp
- Spotify remote control via Web API
- Discord Rich Presence
- Config at `~/.config/noctune/config.toml` (or platform equivalent)
- Themes at `~/.config/noctune/themes/<name>.toml`

## Keybindings

| Key                     | Action                                       |
| ----------------------- | -------------------------------------------- |
| `q` / `Ctrl+C`          | Quit                                         |
| `?`                     | Help overlay                                 |
| `Tab`                   | Switch focus                                 |
| `↑`/`↓` or `j`/`k`     | Move selection                               |
| `Enter`                 | Play selection                               |
| `a`                     | Add to queue                                 |
| `d`                     | Remove from queue                            |
| `c`                     | Clear queue + stop                           |
| `/`                     | Search library (Enter confirms, Esc clears)  |
| `Space`                 | Play / pause                                 |
| `n` / `p`               | Next / previous                              |
| `s`                     | Stop                                         |
| `←` / `→`               | Seek -5s / +5s                               |
| `+` / `-`               | Volume up / down                             |
| `Shift+S`               | Toggle shuffle                               |
| `r`                     | Cycle repeat mode (off / all / one)          |
| `w`                     | Save queue as `.m3u`                         |
| `Shift+L`               | Load most recent `.m3u` from playlists dir   |
| `o`                     | Cycle sort mode (title / artist / album)     |
| `Shift+T`               | Toggle 30-min sleep timer                    |
| `Mouse wheel`           | Scroll selection                             |
| `Mouse click`           | Play library / queue row, seek on progress   |
| `Shift+V`               | Toggle flat / album view                     |
| `1`/`2` `3`/`4` `5`/`6` | EQ low / mid / high -/+ 1 dB               |
| `Shift+P`               | Spotify login (OAuth PKCE)                   |
| `@`                     | Toggle Spotify play/pause                    |
| `v`                     | Cycle visualizer mode                        |
| `[` / `]`               | Visualizer sensitivity down / up             |

Keybinds in the `[keybinds]` section of `config.toml` override the defaults.

## Spotify integration

Noctune can control your active Spotify Connect device via the Web API.

1. Create an app at https://developer.spotify.com/dashboard
2. Add redirect URI `http://127.0.0.1:8888/callback`
3. Put the Client ID in `config.toml`:
   ```toml
   [spotify]
   client_id = "your_client_id"
   redirect_port = 8888
   ```
4. Press `Shift+P` — browser opens, you authorize, tokens are stored automatically.

## Customizing themes

Copy `themes/default.toml` to `themes/<your-theme>.toml`, edit colors, symbols, and the ASCII art logo, then set `theme = "<your-theme>"` in `config.toml`.

## Roadmap

- Embedded Spotify playback via Librespot — [#24](https://github.com/WendellOttoni/Noctune/issues/24)
- YouTube Music via yt-dlp — [#25](https://github.com/WendellOttoni/Noctune/issues/25)
- HTTP radio streaming (Icecast/Shoutcast) — [#20](https://github.com/WendellOttoni/Noctune/issues/20)

See all open issues at https://github.com/WendellOttoni/Noctune/issues
