use crate::library::models::Track;
use crate::utils::error::{AudioError, Result};
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::fs;
use tracing::{info, debug};

#[derive(Debug, Clone)]
pub struct TrackMetadata {
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub genre: Option<String>,
    pub duration: Option<Duration>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    pub bitrate: Option<u32>,
    pub file_size: u64,
    pub modified_time: u64,
}

impl TrackMetadata {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        
        if !path.exists() {
            return Err(AudioError::NotFound(format!("File not found: {}", path.display())));
        }

        let metadata = fs::metadata(path)
            .map_err(|e| AudioError::IoError(format!("Failed to get file metadata: {}", e)))?;

        let file_size = metadata.len();
        let modified_time = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut track_metadata = Self {
            path: path.to_path_buf(),
            title: None,
            artist: None,
            album: None,
            album_artist: None,
            year: None,
            track_number: None,
            disc_number: None,
            genre: None,
            duration: None,
            sample_rate: None,
            bit_depth: None,
            channels: None,
            bitrate: None,
            file_size,
            modified_time,
        };

        let probe_result = Probe::open(path)
            .map_err(|e| AudioError::DecodeError(format!("Failed to probe file: {}", e)))?;

        let tagged_file = probe_result
            .read()
            .map_err(|e| AudioError::DecodeError(format!("Failed to read file: {}", e)))?;

        let properties = tagged_file.properties();
        track_metadata.duration = Some(properties.duration());
        track_metadata.sample_rate = properties.sample_rate();
        track_metadata.bit_depth = properties.bit_depth().map(|b| b as u8);
        track_metadata.channels = properties.channels().map(|c| c as u8);
        track_metadata.bitrate = properties.audio_bitrate();

        let tags = tagged_file.tags();
        if !tags.is_empty() {
            let tag = tags.first().unwrap();

            track_metadata.title = tag.title().map(|s| s.to_string());
            track_metadata.artist = tag.artist().map(|s| s.to_string());
            track_metadata.album = tag.album().map(|s| s.to_string());
            track_metadata.album_artist = tag.get_string(&ItemKey::AlbumArtist).map(|s| s.to_string());
            track_metadata.year = tag.year().map(|y| y as u32);
            track_metadata.genre = tag.genre().map(|s| s.to_string());
            track_metadata.track_number = tag.track().map(|n| n as u32);
            track_metadata.disc_number = tag.disk().map(|n| n as u32);
        }

        if track_metadata.title.is_none() {
            if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                track_metadata.title = Some(filename.to_string());
            }
        }

        if track_metadata.artist.is_none() {
            track_metadata.artist = Some("Unknown Artist".to_string());
        }

        debug!("Extracted metadata for: {}", path.display());
        Ok(track_metadata)
    }

    pub fn apply_to_track(&self, track: &mut Track) {
        track.title = self.title.clone().unwrap_or_else(|| track.display_title());
        track.duration = self.duration;
        track.sample_rate = self.sample_rate;
        track.bit_depth = self.bit_depth;
        track.channels = self.channels;
        track.bitrate = self.bitrate;
        track.file_size = self.file_size;
        track.modified_time = self.modified_time;
        track.track_number = self.track_number;
        track.disc_number = self.disc_number;
        track.genre = self.genre.clone();
        track.year = self.year;
    }
}

pub struct MetadataExtractor {
    cache_enabled: bool,
}

impl MetadataExtractor {
    pub fn new(cache_enabled: bool) -> Self {
        Self { cache_enabled }
    }

    pub fn extract<P: AsRef<Path>>(&self, path: P) -> Result<TrackMetadata> {
        TrackMetadata::from_path(path)
    }

    pub fn extract_to_track<P: AsRef<Path>>(&self, path: P, track: &mut Track) -> Result<()> {
        let metadata = self.extract(path)?;
        metadata.apply_to_track(track);
        Ok(())
    }

    pub fn extract_batch<P: AsRef<Path>>(&self, paths: &[P]) -> Vec<Result<TrackMetadata>> {
        paths.iter().map(|p| self.extract(p)).collect()
    }
}

impl Default for MetadataExtractor {
    fn default() -> Self {
        Self::new(true)
    }
}

pub fn is_audio_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        matches!(
            ext_lower.as_str(),
            "mp3" | "flac" | "wav" | "ogg" | "oga" | "m4a" | "aac" | "wma" | "ape" | "wv"
        )
    } else {
        false
    }
}

pub fn extract_cover_art<P: AsRef<Path>>(path: P) -> Result<Option<Vec<u8>>> {
    let path = path.as_ref();

    let probe_result = Probe::open(path)
        .map_err(|e| AudioError::DecodeError(format!("Failed to probe file: {}", e)))?;

    let tagged_file = probe_result
        .read()
        .map_err(|e| AudioError::DecodeError(format!("Failed to read file: {}", e)))?;

    let tags = tagged_file.tags();
    for tag in tags {
        if let Some(picture) = tag.pictures().first() {
            let picture_data = picture.data().to_vec();
            info!("Found cover art in: {}", path.display());
            return Ok(Some(picture_data));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_file_detection() {
        assert!(is_audio_file(Path::new("test.mp3")));
        assert!(is_audio_file(Path::new("test.flac")));
        assert!(is_audio_file(Path::new("test.wav")));
        assert!(!is_audio_file(Path::new("test.txt")));
        assert!(!is_audio_file(Path::new("test.jpg")));
    }

    #[test]
    fn test_metadata_creation() {
        let metadata = TrackMetadata {
            title: Some("Test Song".to_string()),
            artist: Some("Test Artist".to_string()),
            album: Some("Test Album".to_string()),
            year: Some(2024),
            track_number: Some(1),
            disc_number: Some(1),
            genre: Some("Rock".to_string()),
            duration: Some(Duration::from_secs(180)),
            sample_rate: Some(44100),
            bit_depth: Some(16),
            channels: Some(2),
            bitrate: Some(320),
            file_size: 1024,
            modified_time: 0,
        };

        assert_eq!(metadata.title, Some("Test Song".to_string()));
        assert_eq!(metadata.artist, Some("Test Artist".to_string()));
        assert_eq!(metadata.year, Some(2024));
    }
}
