use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use parking_lot::Mutex;

use crate::audio::Track;

#[derive(Clone)]
pub struct LibraryDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl LibraryDatabase {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path).context("Falha ao abrir banco SQLite da biblioteca")?;

        // Performance pragmas for fast embedded access
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000;
             PRAGMA foreign_keys = ON;",
        )?;

        // Tables: tracks and FTS5 search index
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tracks (
                path TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                artist TEXT,
                album TEXT,
                duration_secs INTEGER,
                genre TEXT,
                year INTEGER,
                track_number INTEGER
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS tracks_fts USING fts5(
                path UNINDEXED,
                title,
                artist,
                album,
                genre,
                tokenize='unicode61 remove_diacritics 2'
            );",
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn sync_tracks(&self, tracks: &[Track]) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        {
            let mut insert_track = tx.prepare_cached(
                "INSERT INTO tracks (path, title, artist, album, duration_secs, genre, year, track_number)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(path) DO UPDATE SET
                    title=excluded.title,
                    artist=excluded.artist,
                    album=excluded.album,
                    duration_secs=excluded.duration_secs,
                    genre=excluded.genre,
                    year=excluded.year,
                    track_number=excluded.track_number",
            )?;

            let mut insert_fts = tx.prepare_cached(
                "INSERT INTO tracks_fts (path, title, artist, album, genre)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;

            // Rebuild FTS index on sync
            tx.execute("DELETE FROM tracks_fts", [])?;

            for t in tracks {
                let path_str = t.path.to_string_lossy().to_string();
                let dur_secs = t.duration.map(|d| d.as_secs() as i64);

                insert_track.execute(params![
                    path_str,
                    t.title,
                    t.artist,
                    t.album,
                    dur_secs,
                    t.genre,
                    t.year,
                    t.track_number
                ])?;

                insert_fts.execute(params![
                    path_str,
                    t.title,
                    t.artist.as_deref().unwrap_or(""),
                    t.album.as_deref().unwrap_or(""),
                    t.genre.as_deref().unwrap_or("")
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn search_fts(&self, query: &str) -> Result<Vec<Track>> {
        let clean_query = query.trim();
        if clean_query.is_empty() {
            return self.load_all();
        }

        // Prepare FTS5 query terms (prefix matching with *)
        let terms: Vec<String> = clean_query
            .split_whitespace()
            .map(|term| {
                let escaped = term.replace('"', "\"\"");
                format!("\"{escaped}\"*")
            })
            .collect();
        let fts_match = terms.join(" AND ");

        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT t.path, t.title, t.artist, t.album, t.duration_secs, t.genre, t.year, t.track_number
             FROM tracks_fts f
             JOIN tracks t ON t.path = f.path
             WHERE tracks_fts MATCH ?1
             ORDER BY rank
             LIMIT 300",
        )?;

        let rows = stmt.query_map(params![fts_match], |row| {
            let path_str: String = row.get(0)?;
            let title: String = row.get(1)?;
            let artist: Option<String> = row.get(2)?;
            let album: Option<String> = row.get(3)?;
            let dur_secs: Option<i64> = row.get(4)?;
            let genre: Option<String> = row.get(5)?;
            let year: Option<i32> = row.get(6)?;
            let track_number: Option<u32> = row.get(7)?;

            Ok(Track {
                path: PathBuf::from(path_str),
                title,
                artist,
                album,
                duration: dur_secs.map(|s| Duration::from_secs(s as u64)),
                genre,
                year,
                track_number,
            })
        })?;

        let mut tracks = Vec::new();
        for r in rows {
            if let Ok(t) = r {
                tracks.push(t);
            }
        }
        Ok(tracks)
    }

    pub fn load_all(&self) -> Result<Vec<Track>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT path, title, artist, album, duration_secs, genre, year, track_number
             FROM tracks
             ORDER BY artist, album, track_number, title",
        )?;

        let rows = stmt.query_map([], |row| {
            let path_str: String = row.get(0)?;
            let title: String = row.get(1)?;
            let artist: Option<String> = row.get(2)?;
            let album: Option<String> = row.get(3)?;
            let dur_secs: Option<i64> = row.get(4)?;
            let genre: Option<String> = row.get(5)?;
            let year: Option<i32> = row.get(6)?;
            let track_number: Option<u32> = row.get(7)?;

            Ok(Track {
                path: PathBuf::from(path_str),
                title,
                artist,
                album,
                duration: dur_secs.map(|s| Duration::from_secs(s as u64)),
                genre,
                year,
                track_number,
            })
        })?;

        let mut tracks = Vec::new();
        for r in rows {
            if let Ok(t) = r {
                tracks.push(t);
            }
        }
        Ok(tracks)
    }

    pub fn count(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached("SELECT COUNT(*) FROM tracks")?;
        let count: usize = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }
}
