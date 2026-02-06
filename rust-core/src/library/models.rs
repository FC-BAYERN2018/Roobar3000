use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::time::Duration;
use crate::audio::format::AudioFormat;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: i64,
    pub path: PathBuf,
    pub title: String,
    pub artist_id: Option<i64>,
    pub album_id: Option<i64>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub duration: Option<Duration>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    pub bitrate: Option<u32>,
    pub file_size: u64,
    pub modified_time: u64,
    pub added_time: u64,
    pub last_played: Option<u64>,
    pub play_count: u32,
    pub rating: Option<u8>,
    pub genre: Option<String>,
    pub year: Option<u32>,
}

impl Track {
    pub fn new(path: PathBuf) -> Self {
        Self {
            id: 0,
            path,
            title: String::new(),
            artist_id: None,
            album_id: None,
            track_number: None,
            disc_number: None,
            duration: None,
            sample_rate: None,
            bit_depth: None,
            channels: None,
            bitrate: None,
            file_size: 0,
            modified_time: 0,
            added_time: 0,
            last_played: None,
            play_count: 0,
            rating: None,
            genre: None,
            year: None,
        }
    }

    pub fn display_title(&self) -> String {
        if !self.title.is_empty() {
            self.title.clone()
        } else {
            self.path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string()
        }
    }

    pub fn format_info(&self) -> String {
        let mut info = String::new();
        
        if let Some(sr) = self.sample_rate {
            info.push_str(&format!("{}Hz", sr));
        }
        
        if let Some(bits) = self.bit_depth {
            if !info.is_empty() {
                info.push_str(", ");
            }
            info.push_str(&format!("{}bit", bits));
        }
        
        if let Some(ch) = self.channels {
            if !info.is_empty() {
                info.push_str(", ");
            }
            info.push_str(&format!("{}ch", ch));
        }
        
        if info.is_empty() {
            info = "Unknown".to_string();
        }
        
        info
    }

    pub fn audio_format(&self) -> Option<AudioFormat> {
        if let (Some(sample_rate), Some(channels), Some(bit_depth)) = 
            (self.sample_rate, self.channels, self.bit_depth) {
            let sample_format = match bit_depth {
                8 => crate::audio::format::SampleFormat::U8,
                16 => crate::audio::format::SampleFormat::S16,
                24 => crate::audio::format::SampleFormat::S24,
                32 => crate::audio::format::SampleFormat::S32,
                _ => crate::audio::format::SampleFormat::S16,
            };
            Some(AudioFormat::new(sample_rate, channels.into(), sample_format))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub id: i64,
    pub name: String,
    pub sort_name: Option<String>,
    pub album_count: u32,
    pub track_count: u32,
    pub added_time: u64,
}

impl Artist {
    pub fn new(name: String) -> Self {
        Self {
            id: 0,
            name,
            sort_name: None,
            album_count: 0,
            track_count: 0,
            added_time: 0,
        }
    }

    pub fn display_name(&self) -> String {
        self.sort_name.as_ref().unwrap_or(&self.name).clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: i64,
    pub title: String,
    pub artist_id: Option<i64>,
    pub artist_name: Option<String>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub track_count: u32,
    pub total_duration: Option<Duration>,
    pub cover_art_path: Option<PathBuf>,
    pub added_time: u64,
}

impl Album {
    pub fn new(title: String) -> Self {
        Self {
            id: 0,
            title,
            artist_id: None,
            artist_name: None,
            year: None,
            genre: None,
            track_count: 0,
            total_duration: None,
            cover_art_path: None,
            added_time: 0,
        }
    }

    pub fn display_title(&self) -> String {
        if let Some(year) = self.year {
            format!("{} ({})", self.title, year)
        } else {
            self.title.clone()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub track_ids: Vec<i64>,
    pub created_time: u64,
    pub modified_time: u64,
}

impl Playlist {
    pub fn new(name: String) -> Self {
        Self {
            id: 0,
            name,
            description: None,
            track_ids: Vec::new(),
            created_time: 0,
            modified_time: 0,
        }
    }

    pub fn add_track(&mut self, track_id: i64) {
        if !self.track_ids.contains(&track_id) {
            self.track_ids.push(track_id);
        }
    }

    pub fn remove_track(&mut self, track_id: i64) {
        self.track_ids.retain(|&id| id != track_id);
    }

    pub fn track_count(&self) -> usize {
        self.track_ids.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryStats {
    pub total_tracks: u64,
    pub total_artists: u64,
    pub total_albums: u64,
    pub total_playlists: u64,
    pub total_duration: Duration,
    pub total_size: u64,
    pub last_scan_time: Option<u64>,
    pub scan_in_progress: bool,
}

impl Default for LibraryStats {
    fn default() -> Self {
        Self {
            total_tracks: 0,
            total_artists: 0,
            total_albums: 0,
            total_playlists: 0,
            total_duration: Duration::ZERO,
            total_size: 0,
            last_scan_time: None,
            scan_in_progress: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub search_in_title: bool,
    pub search_in_artist: bool,
    pub search_in_album: bool,
    pub search_in_genre: bool,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            search_in_title: true,
            search_in_artist: true,
            search_in_album: true,
            search_in_genre: false,
            limit: Some(100),
            offset: Some(0),
        }
    }
}

impl SearchQuery {
    pub fn new(query: String) -> Self {
        Self {
            query,
            ..Default::default()
        }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn search_in_all(mut self) -> Self {
        self.search_in_title = true;
        self.search_in_artist = true;
        self.search_in_album = true;
        self.search_in_genre = true;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortOrder {
    pub field: SortField,
    pub ascending: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SortField {
    Title,
    Artist,
    Album,
    Year,
    Duration,
    AddedTime,
    PlayCount,
    Rating,
}

impl SortOrder {
    pub fn new(field: SortField, ascending: bool) -> Self {
        Self { field, ascending }
    }

    pub fn title_asc() -> Self {
        Self::new(SortField::Title, true)
    }

    pub fn title_desc() -> Self {
        Self::new(SortField::Title, false)
    }

    pub fn artist_asc() -> Self {
        Self::new(SortField::Artist, true)
    }

    pub fn added_time_desc() -> Self {
        Self::new(SortField::AddedTime, false)
    }
}
