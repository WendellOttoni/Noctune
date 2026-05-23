# Shareable Playlist Format

Wire format for playlists that move between Noctune instances or through the
future share backend (#78). The format is **path-agnostic** by design:
a receiver should be able to play a shared playlist without ever seeing the
sender's filesystem.

## Versioning

`schema_version` is a monotonically increasing `u32`. Importing a payload
with a higher version than the local Noctune supports is a hard error
([`ShareError::UnsupportedVersion`]). Old versions remain readable.

Current version: **1**.

## Top-level shape

```json
{
  "schema_version": 1,
  "id": "uuid-v4",
  "name": "Mix de Sexta",
  "description": "",
  "visibility": "public" | "unlisted" | "private",
  "author": { "id": "...", "display_name": "..." },
  "created_at": "RFC3339",
  "updated_at": "RFC3339",
  "tracks": [ ... ]
}
```

`id`, `created_at`, `updated_at` are assigned by the share backend and may be
empty for client-created payloads.

## Tracks

Tagged union on `kind`:

### `kind: "local"`

A track that lives in the sender's library. The receiver resolves it via
metadata against its own scan.

```json
{
  "kind": "local",
  "title": "Song Title",
  "artist": "Artist",
  "album": "Album",
  "duration_ms": 217000,
  "content_hash": "sha256:HEX"
}
```

Only `title` is required. `content_hash` is reserved for exact-match
resolution once a hash index lands.

### `kind: "stream"`

A track that carries its own canonical URL. No library lookup required.

```json
{
  "kind": "stream",
  "title": "Song Title",
  "artist": "Artist",
  "duration_ms": 217000,
  "source": "youtube" | "http",
  "url": "https://..."
}
```

## Resolution strategy

`SharedPlaylist::resolve(library)` walks the tracks in order and produces
`ResolvedItem::{Resolved | Missing}`. For `Local` entries:

1. Try `(title CI, artist CI, duration ±2s)`.
2. Drop the duration constraint.
3. Title-only match.
4. Otherwise → `Missing` (UI should surface).

The duration window is intentionally generous to cover slight transcode
drift between sources.

`Stream` entries always resolve — they synthesise a new local `Track` from
the URL.

## M3U interop

`SharedPlaylist::to_extended_m3u` and `from_extended_m3u` round-trip a
playlist through the `#EXTINF:duration,artist - title` format that Noctune
already understands (see `save_playlist_named` in `src/app.rs`).

Local entries use a `noctune-local://` placeholder in the URL slot — this
lets a downstream tool distinguish "metadata-only local reference" from
"actual playable URL" while still parsing as a valid M3U.

Stream entries write their real URL into the URL slot.

Caveat: M3U does not have first-class fields for visibility, author, or
ids. Use JSON when round-tripping a full `SharedPlaylist`; M3U is for
hand-off to other players.
