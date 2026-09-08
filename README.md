<pre>
       ███▄    █  ▒█████  ▄████▄  ▄▄▄█████▓ █    ██  ███▄    █ ▓█████
       ██ ▀█   █ ▒██▒  ██▒▒██▀ ▀█  ▓  ██▒ ▓▒ ██  ▓██▒ ██ ▀█   █ ▓█   ▀
      ▓██  ▀█ ██▒▒██░  ██▒▒▓█    ▄ ▒ ▓██░ ▒░▓██  ▒██░▓██  ▀█ ██▒▒███
      ▓██▒  ▐▌██▒▒██   ██░▒▓▓▄ ▄██▒░ ▓██▓ ░ ▓▓█  ░██░▓██▒  ▐▌██▒▒▓█  ▄
      ▒██░   ▓██░░ ████▓▒░▒ ▓███▀ ░  ▒██▒ ░ ▒▒█████▓ ▒██░   ▓██░░▒████▒
</pre>

<div align="center">

### Keep the music. Close the browser.

**A fast, customizable music player built for developers who live in the terminal.**

Written in Rust · Keyboard-first · Local music · Streaming · Equalizer · Themes

[Installation](#installation) · [Features](#features) · [Configuration](#configuration) · [Roadmap](#roadmap)

</div>

---

## Why Noctune?

I spend a lot of time programming with music playing in the background.

What I did not like was keeping an entire browser — and sometimes several heavy web processes — running for hours just to listen to music.

So I built **Noctune**.

Noctune is a terminal-native music player designed to stay open next to your editor without requiring a browser tab or a heavyweight desktop interface.

It gives you a full music library, queue, audio visualizers, equalizer, themes, streaming integrations and keyboard-driven controls — directly inside your terminal.

```text
Editor + Terminal + Noctune.

That's it.
```

## Demo

https://github.com/user-attachments/assets/6c468eff-b23e-4064-b64a-c916262e46b4

---

## Installation

### Windows

Run the installer from PowerShell:

```powershell
irm https://raw.githubusercontent.com/WendellOttoni/Noctune/main/install.ps1 | iex
```

Then open a new terminal and run:

```powershell
noctune
```

### Windows — Scoop

```powershell
scoop bucket add noctune https://github.com/WendellOttoni/Noctune
scoop install noctune
```

### Linux / macOS

```sh
curl -fsSL https://raw.githubusercontent.com/WendellOttoni/Noctune/main/install.sh | sh
```

Then:

```sh
noctune
```

Noctune is installed to `~/.local/bin/noctune`.

### Build from source

Requires Rust.

```sh
cargo install --git https://github.com/WendellOttoni/Noctune
```

Or clone the repository:

```sh
git clone https://github.com/WendellOttoni/Noctune
cd Noctune
cargo build --release --locked
```

Run:

```sh
./target/release/noctune
```

### Pre-built binaries

Pre-compiled binaries are also available on the [Releases page](https://github.com/WendellOttoni/Noctune/releases).

---

## Quick start

On the first launch, Noctune can guide you through the initial setup.

You can also configure your music directory manually:

```sh
noctune setup --music "/path/to/music"
```

Then simply run:

```sh
noctune
```

Press:

```text
?
```

at any time to open the help interface.

---

## Features

### 🎵 Playback

* MP3
* FLAC
* WAV
* OGG
* Opus
* M4A
* AAC
* Local music libraries
* Playback queue
* Now-playing view
* Seek controls
* Pause / resume
* Audio caching for downloaded streams

Playback is powered by **Symphonia** and **Rodio**.

### 📊 Audio visualization

Noctune includes terminal-native audio visualization:

* Waveform visualization
* Spectrum visualization
* Live playback feedback

No browser canvas. No desktop GUI.

Just your terminal.

### 🎚 Equalizer

Built-in **3-band equalizer** with configurable presets.

Adjust your sound without depending on an external audio player.

### 🎨 Themes

The entire interface can be customized through TOML theme files.

Themes can control:

* Colors
* Symbols
* Borders
* ASCII artwork
* UI appearance

Theme files live at:

```text
~/.config/noctune/themes/
```

Create your own:

```sh
cp themes/default.toml themes/my-theme.toml
```

Then configure:

```toml
theme = "my-theme"
```

Noctune is designed to look like it belongs in **your terminal**.

### ⌨ Keyboard-first interface

Noctune is built around keyboard navigation.

Press:

```text
?
```

to view available shortcuts.

You can also open the command palette or export the currently configured command registry:

```sh
noctune commands
```

Keybindings can be overridden through configuration.

### 🌐 Streaming

Noctune supports HTTP and YouTube-based streaming through `yt-dlp`.

> YouTube streaming requires [`yt-dlp`](https://github.com/yt-dlp/yt-dlp) to be installed and available in your `PATH`.

Streaming audio can be cached locally according to configurable storage and expiration limits.

### 🟢 Spotify integration

Noctune can control an active **Spotify Connect** device through the Spotify Web API.

You can:

* Authenticate with Spotify
* Control playback
* Interact with your active Spotify device
* Use Spotify from the Noctune interface

> Noctune currently controls Spotify remotely. Native Spotify audio playback inside Noctune is not currently implemented.

### 🎮 Discord Rich Presence

Optionally display what you are listening to through Discord Rich Presence.

### 🌎 Language support

The interface, dialogs, command palette, notifications, CLI help and diagnostics support:

```text
English
Português do Brasil
```

Configure:

```toml
[ui]
language = "pt-BR"
```

or:

```toml
[ui]
language = "en"
```

English is the default. Open the command palette with `Ctrl+P` and run
`:language` to switch languages and save the preference. Library headings and
other interface labels update without restarting, including after editing
`[ui].language` in `config.toml`.

Track metadata, station names, user-created names, technical error details from
services/libraries, and log output retain their original text. Configuration
keys, command IDs, and JSON output stay stable in both languages.

---

## Built for the terminal

Noctune uses:

* **Rust**
* **Ratatui**
* **Crossterm**
* **Rodio**
* **Symphonia**

The goal is not to recreate a desktop music player inside a terminal.

The goal is to provide the things you actually need while working:

```text
Music
Library
Queue
Streaming
Equalizer
Visualizer
Themes
Keyboard controls
```

without requiring another large graphical application to stay open beside your development environment.

---

## Configuration

Configuration is stored in:

```text
~/.config/noctune/config.toml
```

or the equivalent platform-specific configuration directory.

Themes are stored in:

```text
~/.config/noctune/themes/
```

Example UI configuration:

```toml
[ui]
language = "en"
simple_symbols = false
```

For terminals with limited Unicode support:

```toml
[ui]
simple_symbols = true
```

### Audio cache

Downloaded audio has independent cache limits.

Example:

```toml
[cache]
audio_max_size_mb = 1024
audio_expire_days = 30
```

Cache inspection and cleanup are available through the command palette.

Files currently being used for playback are protected from cleanup.

---

## Spotify setup

Noctune uses Spotify's Web API to control your active Spotify Connect device.

### 1. Create an application

Go to the [Spotify Developer Dashboard](https://developer.spotify.com/dashboard).

### 2. Add the redirect URI

```text
http://127.0.0.1:8888/callback
```

### 3. Configure Noctune

Add your Client ID:

```toml
[spotify]
client_id = "your_client_id"
redirect_port = 8888
```

### 4. Authenticate

Inside Noctune, press:

```text
Shift + P
```

Your browser will open for authorization.

After authentication, Noctune stores the required tokens automatically.

---

## Diagnostics

Something not working?

Run:

```sh
noctune doctor
```

For machine-readable diagnostics:

```sh
noctune doctor --json
```

To test audio output explicitly:

```sh
noctune doctor --test-audio
```

Normal diagnostics do **not** play audio.

The JSON diagnostic output is designed to avoid exposing credentials, URLs, usernames and local paths.

---

## Project status

Noctune is actively evolving.

The project includes automated tests covering areas such as:

* Playback behavior
* Credentials
* Cache handling
* HTTP behavior
* Spotify contracts
* Queue management
* Unicode layout behavior
* Process handling
* Update validation

Development hardening and platform-specific validation are still ongoing.

For the detailed implementation state and known limitations, see:

[Implementation status](docs/implementation-status.md)

For the engineering review behind recent improvements:

[Noctune improvement report](docs/melhorias-noctune-2026-09-07.md)

---

## Release integrity

Noctune's current release workflow includes additional integrity protections.

The updater and installers expect matching `.sha256` sidecar files and preserve a backup while replacing an installation.

Releases built through the updated workflow also produce build attestations.

Older releases that do not provide the expected checksum may be intentionally rejected by the new updater.

> Checksums protect against accidental corruption. They do not protect against a compromised publisher or release infrastructure.

---

## Roadmap

Some of the next areas being explored:

* [Embedded Spotify playback via Librespot](https://github.com/WendellOttoni/Noctune/issues/24)
* [YouTube Music support](https://github.com/WendellOttoni/Noctune/issues/25)
* [Icecast / Shoutcast internet radio](https://github.com/WendellOttoni/Noctune/issues/20)

See all open issues:

https://github.com/WendellOttoni/Noctune/issues

---

## Contributing

Contributions are welcome.

If you find a bug, have an idea or want to improve Noctune, feel free to:

* Open an issue
* Suggest a feature
* Submit a pull request
* Improve documentation
* Create a theme

Before making larger architectural changes, opening an issue first is recommended so the idea can be discussed.

---

## Philosophy

Noctune started from a simple idea:

> **Music should not require a heavyweight application sitting beside your development environment all day.**

Noctune is built for people who already spend most of their time inside editors, shells and terminals.

No accounts are required for local playback.

No browser UI is required for your local library.

Open the terminal.

Start Noctune.

Keep coding.

---

## License

Noctune is open source and distributed under the [MIT License](LICENSE).

---

<div align="center">

### Keep the music. Close the browser.

If Noctune is useful to you, consider giving the repository a ⭐.

It helps more developers discover the project.

**[⭐ Star Noctune](https://github.com/WendellOttoni/Noctune)**

</div>
