use crate::library::models::{Track, Artist, Album, Playlist, LibraryStats, SearchQuery};
use crate::library::metadata::TrackMetadata;
use crate::utils::error::{AudioError, Result};
use rusqlite::{Connection, params, OptionalExtension};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub read_only: bool,
}

impl DatabaseConfig {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            read_only: false,
        }
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }
}

pub struct LibraryDatabase {
    conn: Connection,
    config: DatabaseConfig,
}

impl LibraryDatabase {
    pub fn new(config: DatabaseConfig) -> Result<Self> {
        let flags = if config.read_only {
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
        };

        let conn = Connection::open_with_flags(&config.path, flags)
            .map_err(|e| AudioError::IoError(format!("Failed to open database: {}", e)))?;

        info!("Library database opened: {}", config.path.display());

        let db = Self { conn, config };
        
        if !db.config.read_only {
            db.init_schema()?;
        }

        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            BEGIN;

            CREATE TABLE IF NOT EXISTS artists (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                sort_name TEXT,
                album_count INTEGER DEFAULT 0,
                track_count INTEGER DEFAULT 0,
                added_time INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );

            CREATE TABLE IF NOT EXISTS albums (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                artist_id INTEGER,
                artist_name TEXT,
                year INTEGER,
                genre TEXT,
                track_count INTEGER DEFAULT 0,
                total_duration INTEGER,
                cover_art_path TEXT,
                added_time INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE SET NULL
            );

            CREATE TABLE IF NOT EXISTS tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                artist_id INTEGER,
                album_id INTEGER,
                track_number INTEGER,
                disc_number INTEGER,
                duration INTEGER,
                sample_rate INTEGER,
                bit_depth INTEGER,
                channels INTEGER,
                bitrate INTEGER,
                file_size INTEGER NOT NULL,
                modified_time INTEGER NOT NULL,
                added_time INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                last_played INTEGER,
                play_count INTEGER DEFAULT 0,
                rating INTEGER,
                genre TEXT,
                year INTEGER,
                FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE SET NULL,
                FOREIGN KEY (album_id) REFERENCES albums(id) ON DELETE SET NULL
            );

            CREATE TABLE IF NOT EXISTS playlists (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                created_time INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                modified_time INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );

            CREATE TABLE IF NOT EXISTS playlist_tracks (
                playlist_id INTEGER NOT NULL,
                track_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                PRIMARY KEY (playlist_id, track_id),
                FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
                FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_tracks_path ON tracks(path);
            CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist_id);
            CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album_id);
            CREATE INDEX IF NOT EXISTS idx_tracks_title ON tracks(title COLLATE NOCASE);
            CREATE INDEX IF NOT EXISTS idx_albums_artist ON albums(artist_id);
            CREATE INDEX IF NOT EXISTS idx_albums_title ON albums(title COLLATE NOCASE);
            CREATE INDEX IF NOT EXISTS idx_artists_name ON artists(name COLLATE NOCASE);

            COMMIT;
            "#
        ).map_err(|e| AudioError::IoError(format!("Failed to initialize database schema: {}", e)))?;

        info!("Database schema initialized");
        Ok(())
    }

    pub fn begin_transaction(&self) -> Result<()> {
        self.conn.execute("BEGIN TRANSACTION", [])
            .map_err(|e| AudioError::IoError(format!("Failed to begin transaction: {}", e)))?;
        Ok(())
    }

    pub fn commit(&self) -> Result<()> {
        self.conn.execute("COMMIT", [])
            .map_err(|e| AudioError::IoError(format!("Failed to commit transaction: {}", e)))?;
        Ok(())
    }

    pub fn rollback(&self) -> Result<()> {
        self.conn.execute("ROLLBACK", [])
            .map_err(|e| AudioError::IoError(format!("Failed to rollback transaction: {}", e)))?;
        Ok(())
    }

    pub fn add_or_get_artist(&self, name: &str) -> Result<i64> {
        if let Some(id) = self.find_artist_id(name)? {
            return Ok(id);
        }

        self.conn.execute(
            "INSERT INTO artists (name) VALUES (?1)",
            params![name]
        ).map_err(|e| AudioError::IoError(format!("Failed to insert artist: {}", e)))?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn find_artist_id(&self, name: &str) -> Result<Option<i64>> {
        self.conn.query_row(
            "SELECT id FROM artists WHERE name = ?1",
            params![name],
            |row| row.get(0)
        ).optional().map_err(|e| AudioError::IoError(format!("Failed to find artist: {}", e)))
    }

    pub fn add_or_get_album(&self, title: &str, artist_id: Option<i64>, year: Option<u32>) -> Result<i64> {
        if let Some(id) = self.find_album_id(title, artist_id)? {
            return Ok(id);
        }

        self.conn.execute(
            "INSERT INTO albums (title, artist_id, year) VALUES (?1, ?2, ?3)",
            params![title, artist_id, year]
        ).map_err(|e| AudioError::IoError(format!("Failed to insert album: {}", e)))?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn find_album_id(&self, title: &str, artist_id: Option<i64>) -> Result<Option<i64>> {
        let query = if artist_id.is_some() {
            "SELECT id FROM albums WHERE title = ?1 AND artist_id = ?2"
        } else {
            "SELECT id FROM albums WHERE title = ?1 AND artist_id IS NULL"
        };

        self.conn.query_row(
            query,
            params![title, artist_id],
            |row| row.get(0)
        ).optional().map_err(|e| AudioError::IoError(format!("Failed to find album: {}", e)))
    }

    pub fn add_track(&self, metadata: &TrackMetadata) -> Result<i64> {
        let artist_id = metadata.artist.as_ref()
            .map(|a| self.add_or_get_artist(a))
            .transpose()?;

        let album_id = metadata.album.as_ref()
            .map(|a| self.add_or_get_album(a, artist_id, metadata.year))
            .transpose()?;

        self.conn.execute(
            r#"
            INSERT INTO tracks (
                path, title, artist_id, album_id, track_number, disc_number,
                duration, sample_rate, bit_depth, channels, bitrate,
                file_size, modified_time, genre, year
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                artist_id = excluded.artist_id,
                album_id = excluded.album_id,
                track_number = excluded.track_number,
                disc_number = excluded.disc_number,
                duration = excluded.duration,
                sample_rate = excluded.sample_rate,
                bit_depth = excluded.bit_depth,
                channels = excluded.channels,
                bitrate = excluded.bitrate,
                file_size = excluded.file_size,
                modified_time = excluded.modified_time,
                genre = excluded.genre,
                year = excluded.year
            "#,
            params![
                metadata.path.display().to_string(),
                metadata.title.as_ref().unwrap_or(&String::new()),
                artist_id,
                album_id,
                metadata.track_number,
                metadata.disc_number,
                metadata.duration.map(|d| d.as_secs()),
                metadata.sample_rate,
                metadata.bit_depth.map(|b| b as i32),
                metadata.channels.map(|c| c as i32),
                metadata.bitrate.map(|b| b as i32),
                metadata.file_size as i64,
                metadata.modified_time as i64,
                metadata.genre,
                metadata.year.map(|y| y as i32),
            ]
        ).map_err(|e| AudioError::IoError(format!("Failed to insert track: {}", e)))?;

        let track_id = self.conn.last_insert_rowid();

        if let Some(album_id) = album_id {
            self.update_album_track_count(album_id)?;
        }

        if let Some(artist_id) = artist_id {
            self.update_artist_track_count(artist_id)?;
        }

        Ok(track_id)
    }

    pub fn update_album_track_count(&self, album_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE albums SET track_count = (SELECT COUNT(*) FROM tracks WHERE album_id = ?1) WHERE id = ?1",
            params![album_id]
        ).map_err(|e| AudioError::IoError(format!("Failed to update album track count: {}", e)))?;
        Ok(())
    }

    pub fn update_artist_track_count(&self, artist_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE artists SET track_count = (SELECT COUNT(*) FROM tracks WHERE artist_id = ?1) WHERE id = ?1",
            params![artist_id]
        ).map_err(|e| AudioError::IoError(format!("Failed to update artist track count: {}", e)))?;
        Ok(())
    }

    pub fn get_track_by_path<P: AsRef<Path>>(&self, path: P) -> Result<Option<Track>> {
        let path_str = path.as_ref().display().to_string();
        
        self.conn.query_row(
            "SELECT * FROM tracks WHERE path = ?1",
            params![path_str],
            |row| self.row_to_track(row)
        ).optional().map_err(|e| AudioError::IoError(format!("Failed to get track: {}", e)))
    }

    pub fn get_track_by_id(&self, id: i64) -> Result<Option<Track>> {
        self.conn.query_row(
            "SELECT * FROM tracks WHERE id = ?1",
            params![id],
            |row| self.row_to_track(row)
        ).optional().map_err(|e| AudioError::IoError(format!("Failed to get track: {}", e)))
    }

    pub fn get_all_tracks(&self, limit: Option<usize>, offset: Option<usize>) -> Result<Vec<Track>> {
        let mut query = "SELECT * FROM tracks".to_string();
        
        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }
        
        if let Some(offset) = offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = self.conn.prepare(&query)
            .map_err(|e| AudioError::IoError(format!("Failed to prepare query: {}", e)))?;

        let tracks = stmt.query_map([], |row| self.row_to_track(row))
            .map_err(|e| AudioError::IoError(format!("Failed to query tracks: {}", e)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AudioError::IoError(format!("Failed to collect tracks: {}", e)))?;

        Ok(tracks)
    }

    pub fn search_tracks(&self, query: &SearchQuery) -> Result<Vec<Track>> {
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        if !query.query.is_empty() {
            let search_term = format!("%{}%", query.query);
            
            if query.search_in_title {
                conditions.push("title LIKE ?");
                params.push(search_term.clone());
            }
            
            if query.search_in_artist {
                conditions.push("artist_id IN (SELECT id FROM artists WHERE name LIKE ?)");
                params.push(search_term.clone());
            }
            
            if query.search_in_album {
                conditions.push("album_id IN (SELECT id FROM albums WHERE title LIKE ?)");
                params.push(search_term.clone());
            }
            
            if query.search_in_genre {
                conditions.push("genre LIKE ?");
                params.push(search_term);
            }
        }

        let sql = if conditions.is_empty() {
            "SELECT * FROM tracks".to_string()
        } else {
            format!("SELECT * FROM tracks WHERE {}", conditions.join(" OR "))
        };

        let mut stmt = self.conn.prepare(&sql)
            .map_err(|e| AudioError::IoError(format!("Failed to prepare search query: {}", e)))?;

        let tracks = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| self.row_to_track(row))
            .map_err(|e| AudioError::IoError(format!("Failed to execute search: {}", e)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AudioError::IoError(format!("Failed to collect tracks: {}", e)))?;

        Ok(tracks)
    }

    pub fn get_tracks_by_album(&self, album_id: i64) -> Result<Vec<Track>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM tracks WHERE album_id = ?1 ORDER BY disc_number, track_number"
        ).map_err(|e| AudioError::IoError(format!("Failed to prepare query: {}", e)))?;

        let tracks = stmt.query_map(params![album_id], |row| self.row_to_track(row))
            .map_err(|e| AudioError::IoError(format!("Failed to query tracks: {}", e)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AudioError::IoError(format!("Failed to collect tracks: {}", e)))?;

        Ok(tracks)
    }

    pub fn get_tracks_by_artist(&self, artist_id: i64) -> Result<Vec<Track>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM tracks WHERE artist_id = ?1 ORDER BY album_id, disc_number, track_number"
        ).map_err(|e| AudioError::IoError(format!("Failed to prepare query: {}", e)))?;

        let tracks = stmt.query_map(params![artist_id], |row| self.row_to_track(row))
            .map_err(|e| AudioError::IoError(format!("Failed to query tracks: {}", e)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AudioError::IoError(format!("Failed to collect tracks: {}", e)))?;

        Ok(tracks)
    }

    pub fn get_all_artists(&self) -> Result<Vec<Artist>> {
        let mut stmt = self.conn.prepare("SELECT * FROM artists ORDER BY name")
            .map_err(|e| AudioError::IoError(format!("Failed to prepare query: {}", e)))?;

        let artists = stmt.query_map([], |row| self.row_to_artist(row))
            .map_err(|e| AudioError::IoError(format!("Failed to query artists: {}", e)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AudioError::IoError(format!("Failed to collect artists: {}", e)))?;

        Ok(artists)
    }

    pub fn get_all_albums(&self) -> Result<Vec<Album>> {
        let mut stmt = self.conn.prepare("SELECT * FROM albums ORDER BY year DESC, title")
            .map_err(|e| AudioError::IoError(format!("Failed to prepare query: {}", e)))?;

        let albums = stmt.query_map([], |row| self.row_to_album(row))
            .map_err(|e| AudioError::IoError(format!("Failed to query albums: {}", e)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AudioError::IoError(format!("Failed to collect albums: {}", e)))?;

        Ok(albums)
    }

    pub fn get_albums_by_artist(&self, artist_id: i64) -> Result<Vec<Album>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM albums WHERE artist_id = ?1 ORDER BY year DESC, title"
        ).map_err(|e| AudioError::IoError(format!("Failed to prepare query: {}", e)))?;

        let albums = stmt.query_map(params![artist_id], |row| self.row_to_album(row))
            .map_err(|e| AudioError::IoError(format!("Failed to query albums: {}", e)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AudioError::IoError(format!("Failed to collect albums: {}", e)))?;

        Ok(albums)
    }

    pub fn delete_track<P: AsRef<Path>>(&self, path: P) -> Result<bool> {
        let path_str = path.as_ref().display().to_string();
        
        let rows_affected = self.conn.execute(
            "DELETE FROM tracks WHERE path = ?1",
            params![path_str]
        ).map_err(|e| AudioError::IoError(format!("Failed to delete track: {}", e)))?;

        Ok(rows_affected > 0)
    }

    pub fn update_track_play_count(&self, track_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE tracks SET play_count = play_count + 1, last_played = strftime('%s', 'now') WHERE id = ?1",
            params![track_id]
        ).map_err(|e| AudioError::IoError(format!("Failed to update play count: {}", e)))?;
        Ok(())
    }

    pub fn create_playlist(&self, name: &str, description: Option<&str>) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO playlists (name, description) VALUES (?1, ?2)",
            params![name, description]
        ).map_err(|e| AudioError::IoError(format!("Failed to create playlist: {}", e)))?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_all_playlists(&self) -> Result<Vec<Playlist>> {
        let mut stmt = self.conn.prepare("SELECT * FROM playlists ORDER BY name")
            .map_err(|e| AudioError::IoError(format!("Failed to prepare query: {}", e)))?;

        let playlists = stmt.query_map([], |row| self.row_to_playlist(row))
            .map_err(|e| AudioError::IoError(format!("Failed to query playlists: {}", e)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AudioError::IoError(format!("Failed to collect playlists: {}", e)))?;

        Ok(playlists)
    }

    pub fn add_track_to_playlist(&self, playlist_id: i64, track_id: i64) -> Result<()> {
        let max_position: Option<i64> = self.conn.query_row(
            "SELECT MAX(position) FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |row| row.get(0)
        ).optional().map_err(|e| AudioError::IoError(format!("Failed to get max position: {}", e)))?;

        let position = max_position.map_or(0, |p| p + 1);

        self.conn.execute(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
            params![playlist_id, track_id, position]
        ).map_err(|e| AudioError::IoError(format!("Failed to add track to playlist: {}", e)))?;

        Ok(())
    }

    pub fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<Track>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.* FROM tracks t 
             INNER JOIN playlist_tracks pt ON t.id = pt.track_id 
             WHERE pt.playlist_id = ?1 
             ORDER BY pt.position"
        ).map_err(|e| AudioError::IoError(format!("Failed to prepare query: {}", e)))?;

        let tracks = stmt.query_map(params![playlist_id], |row| self.row_to_track(row))
            .map_err(|e| AudioError::IoError(format!("Failed to query playlist tracks: {}", e)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AudioError::IoError(format!("Failed to collect playlist tracks: {}", e)))?;

        Ok(tracks)
    }

    pub fn get_stats(&self) -> Result<LibraryStats> {
        let total_tracks: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tracks",
            [],
            |row| row.get(0)
        ).map_err(|e| AudioError::IoError(format!("Failed to get track count: {}", e)))?;

        let total_artists: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM artists",
            [],
            |row| row.get(0)
        ).map_err(|e| AudioError::IoError(format!("Failed to get artist count: {}", e)))?;

        let total_albums: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM albums",
            [],
            |row| row.get(0)
        ).map_err(|e| AudioError::IoError(format!("Failed to get album count: {}", e)))?;

        let total_playlists: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM playlists",
            [],
            |row| row.get(0)
        ).map_err(|e| AudioError::IoError(format!("Failed to get playlist count: {}", e)))?;

        let total_duration: u64 = self.conn.query_row(
            "SELECT COALESCE(SUM(duration), 0) FROM tracks",
            [],
            |row| row.get(0)
        ).map_err(|e| AudioError::IoError(format!("Failed to get total duration: {}", e)))?;

        let total_size: u64 = self.conn.query_row(
            "SELECT SUM(file_size) FROM tracks",
            [],
            |row| row.get(0)
        ).map_err(|e| AudioError::IoError(format!("Failed to get total size: {}", e)))?;

        Ok(LibraryStats {
            total_tracks,
            total_artists,
            total_albums,
            total_playlists,
            total_duration: Duration::from_secs(total_duration),
            total_size,
            last_scan_time: None,
            scan_in_progress: false,
        })
    }

    fn row_to_track(&self, row: &rusqlite::Row) -> rusqlite::Result<Track> {
        Ok(Track {
            id: row.get(0)?,
            path: PathBuf::from(row.get::<_, String>(1)?),
            title: row.get(2)?,
            artist_id: row.get(3)?,
            album_id: row.get(4)?,
            track_number: row.get(5)?,
            disc_number: row.get(6)?,
            duration: row.get::<_, Option<i64>>(7)?.map(|s| Duration::from_secs(s as u64)),
            sample_rate: row.get(8)?,
            bit_depth: row.get::<_, Option<i32>>(9)?.map(|b| b as u8),
            channels: row.get::<_, Option<i32>>(10)?.map(|c| c as u8),
            bitrate: row.get::<_, Option<i32>>(11)?.map(|b| b as u32),
            file_size: row.get::<_, i64>(12)? as u64,
            modified_time: row.get::<_, i64>(13)? as u64,
            added_time: row.get::<_, i64>(14)? as u64,
            last_played: row.get::<_, Option<i64>>(15)?.map(|t| t as u64),
            play_count: row.get::<_, i32>(16)? as u32,
            rating: row.get::<_, Option<i32>>(17)?.map(|r| r as u8),
            genre: row.get(18)?,
            year: row.get::<_, Option<i32>>(19)?.map(|y| y as u32),
        })
    }

    fn row_to_artist(&self, row: &rusqlite::Row) -> rusqlite::Result<Artist> {
        Ok(Artist {
            id: row.get(0)?,
            name: row.get(1)?,
            sort_name: row.get(2)?,
            album_count: row.get::<_, i32>(3)? as u32,
            track_count: row.get::<_, i32>(4)? as u32,
            added_time: row.get::<_, i64>(5)? as u64,
        })
    }

    fn row_to_album(&self, row: &rusqlite::Row) -> rusqlite::Result<Album> {
        Ok(Album {
            id: row.get(0)?,
            title: row.get(1)?,
            artist_id: row.get(2)?,
            artist_name: row.get(3)?,
            year: row.get::<_, Option<i32>>(4)?.map(|y| y as u32),
            genre: row.get(5)?,
            track_count: row.get::<_, i32>(6)? as u32,
            total_duration: row.get::<_, Option<i64>>(7)?.map(|s| Duration::from_secs(s as u64)),
            cover_art_path: row.get::<_, Option<String>>(8)?.map(PathBuf::from),
            added_time: row.get::<_, i64>(9)? as u64,
        })
    }

    fn row_to_playlist(&self, row: &rusqlite::Row) -> rusqlite::Result<Playlist> {
        let playlist_id: i64 = row.get(0)?;
        
        let track_ids: Vec<i64> = {
            let mut stmt = self.conn.prepare(
                "SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position"
            )?;

            let rows = stmt.query_map(params![playlist_id], |row| row.get(0))?;

            rows.filter_map(|r| r.ok()).collect()
        };

        Ok(Playlist {
            id: playlist_id,
            name: row.get(1)?,
            description: row.get(2)?,
            track_ids,
            created_time: row.get::<_, i64>(3)? as u64,
            modified_time: row.get::<_, i64>(4)? as u64,
        })
    }
}
