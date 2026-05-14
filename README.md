# Noctune

A fully customizable terminal music player in Rust. ASCII-art TUI, multi-format playback, theme system via TOML.

> Status: early MVP — plays local files, queue/library navigation, themable UI.

## Features

- Plays MP3, FLAC, WAV, OGG, Opus, M4A, AAC (via Symphonia + rodio)
- TUI built on ratatui + crossterm
- Library / queue / now-playing panes with focus-aware borders
- Theme files in TOML: colors, symbols, ASCII art logo
- Config at `~/.config/noctune/config.toml` (or platform equivalent)
- Themes at `~/.config/noctune/themes/<name>.toml`

## Build

```sh
cargo run --release
```

On first launch, Noctune writes a default `config.toml` and `themes/default.toml`. Edit `music_dirs` in the config to point at your library.

## Keybindings

| Key                | Action                                  |
| ------------------ | --------------------------------------- |
| `q` / `Ctrl+C`     | Quit                                    |
| `?`                | Help overlay                            |
| `Tab`              | Switch focus                            |
| `↑`/`↓` or `j`/`k` | Move selection                          |
| `Enter`            | Play selection                          |
| `a`                | Add to queue                            |
| `d`                | Remove from queue                       |
| `c`                | Clear queue + stop                      |
| `/`                | Search library (Enter confirms, Esc clears) |
| `Space`            | Play / pause                            |
| `n` / `p`          | Next / previous                         |
| `s`                | Stop                                    |
| `←` / `→`          | Seek -5s / +5s                          |
| `+` / `-`          | Volume up / down                        |
| `Shift+S`          | Toggle shuffle                          |
| `r`                | Cycle repeat mode (off / all / one)     |
| `w`                | Save queue as `.m3u`                    |
| `Shift+L`          | Load most recent `.m3u` from playlists dir |
| `o`                | Cycle sort mode (title / artist / album) |
| `Shift+T`          | Toggle 30-min sleep timer               |
| `Mouse wheel`      | Scroll selection                        |
| `Shift+P`          | Spotify login (OAuth PKCE)              |
| `@`                | Toggle Spotify play/pause               |

Keybinds in the `[keybinds]` section of `config.toml` override the defaults (e.g. `quit = "Ctrl+x"`). Modifier prefixes: `Ctrl+`, `Shift+`, `Alt+`. Special names: `space`, `enter`, `tab`, `esc`, `backspace`, `up`/`down`/`left`/`right`.

## Spotify integration (remote-control)

Noctune can control your active Spotify Connect device via the Web API (no audio is decoded by Noctune — playback happens wherever Spotify is open).

1. Create an app at https://developer.spotify.com/dashboard
2. Add redirect URI `http://127.0.0.1:8888/callback`
3. Put the Client ID in `config.toml`:
   ```toml
   [spotify]
   client_id = "your_client_id"
   redirect_port = 8888
   ```
4. Press `Shift+P` in Noctune — browser opens, you authorize, tokens are stored in `spotify-tokens.json` next to the config.

Embedded playback (Librespot) tracked in [#24](https://github.com/WendellOttoni/Noctune/issues/24). YouTube Music in [#25](https://github.com/WendellOttoni/Noctune/issues/25).

## Customizing themes

Copy `themes/default.toml` to `themes/<your-theme>.toml`, edit colors, symbols, and the ASCII art logo, then set `theme = "<your-theme>"` in `config.toml`.

## Roadmap

- Embedded Spotify playback via Librespot (Premium required) — [#24](https://github.com/WendellOttoni/Noctune/issues/24)
- YouTube Music via yt-dlp / ytmusicapi — [#25](https://github.com/WendellOttoni/Noctune/issues/25)
- HTTP radio streaming (Icecast/Shoutcast) — [#20](https://github.com/WendellOttoni/Noctune/issues/20)
- Crossfade between tracks — [#26](https://github.com/WendellOttoni/Noctune/issues/26)
- 3-band EQ — [#27](https://github.com/WendellOttoni/Noctune/issues/27)
- Album/artist hierarchical view — [#18](https://github.com/WendellOttoni/Noctune/issues/18)
- Click-to-play (currently only scroll works) — [#11](https://github.com/WendellOttoni/Noctune/issues/11)
- Lua-scripted layouts

See open issues at https://github.com/WendellOttoni/Noctune/issues
