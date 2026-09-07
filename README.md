<pre>
       ███▄    █  ▒█████  ▄████▄  ▄▄▄█████▓ █    ██  ███▄    █ ▓█████
       ██ ▀█   █ ▒██▒  ██▒▒██▀ ▀█  ▓  ██▒ ▓▒ ██  ▓██▒ ██ ▀█   █ ▓█   ▀
      ▓██  ▀█ ██▒▒██░  ██▒▒▓█    ▄ ▒ ▓██░ ▒░▓██  ▒██░▓██  ▀█ ██▒▒███
      ▓██▒  ▐▌██▒▒██   ██░▒▓▓▄ ▄██▒░ ▓██▓ ░ ▓▓█  ░██░▓██▒  ▐▌██▒▒▓█  ▄
      ▒██░   ▓██░░ ████▓▒░▒ ▓███▀ ░  ▒██▒ ░ ▒▒█████▓ ▒██░   ▓██░░▒████▒
</pre>

A fully customizable terminal music player in Rust. ASCII-art TUI, multi-format playback, theme system via TOML.

## Demo


https://github.com/user-attachments/assets/6c468eff-b23e-4064-b64a-c916262e46b4


> Development hardening is not yet released. See [implementation status](docs/implementation-status.md).

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
cargo build --release --locked
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

Press `?` for help or open the command palette. Help, palette actions and the
`noctune commands` Markdown export share one registry and reflect your configured
`[keybinds]` overrides.

## Setup and diagnostics

```sh
noctune setup --music "/path/to/music"
noctune doctor
noctune doctor --json
noctune commands
```

Interactive first launch offers setup. `doctor --test-audio` optionally plays a short
tone; normal diagnostics do not play audio. JSON diagnostics omit credentials,
URLs, usernames and local paths.

In your existing configuration, set `language = "pt-BR"` (or `"en"`) and
`simple_symbols = true` under `[ui]` for the core UI. Some integration messages
remain untranslated. Audio downloads have separate `[cache]` settings:
`audio_max_size_mb = 1024` and `audio_expire_days = 30`.
Cache status and cleanup are available in the palette; active files are protected.

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
## Release integrity

The new updater and installers require a matching `.sha256` sidecar and preserve
a backup during replacement. Older releases without that file are intentionally
refused; build from source until a release from the new workflow is published.
Checksums detect corruption, not a compromised publisher. The workflow also
produces build attestations. Native Spotify audio is not implemented: use an
active Spotify Connect device.

See [the research report](docs/melhorias-noctune-2026-09-07.md) and
[implementation status](docs/implementation-status.md) for coverage and limitations.
