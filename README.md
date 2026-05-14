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

## Customizing themes

Copy `themes/default.toml` to `themes/<your-theme>.toml`, edit colors, symbols, and the ASCII art logo, then set `theme = "<your-theme>"` in `config.toml`.

## Roadmap

- Streaming sources (Bandcamp / SoundCloud / radio streams)
- Lua-scripted layouts
- Better metadata caching (avoid re-probing the whole library each launch)
- Embedded cover art preview (chafa-style)

See open issues at https://github.com/WendellOttoni/Noctune/issues
