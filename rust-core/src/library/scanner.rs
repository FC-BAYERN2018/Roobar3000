use crate::library::database::LibraryDatabase;
use crate::library::metadata::{MetadataExtractor, is_audio_file};
use crate::library::models::LibraryStats;
use crate::utils::error::{AudioError, Result};
use crossbeam_channel::Sender;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::fs;
use std::time::{Duration, Instant};
use tracing::{info, debug, warn};

#[derive(Debug, Clone)]
pub enum ScanProgress {
    Started { total_files: usize },
    Progress { current: usize, total: usize, file: PathBuf },
    Completed { result: ScanResult },
    Error { file: PathBuf, error: String },
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub total_files_scanned: usize,
    pub new_tracks: usize,
    pub updated_tracks: usize,
    pub removed_tracks: usize,
    pub failed_files: usize,
    pub duration: Duration,
    pub stats: LibraryStats,
}

#[derive(Debug, Clone)]
pub struct ScanConfig {
    pub directories: Vec<PathBuf>,
    pub recursive: bool,
    pub incremental: bool,
    pub parallel: bool,
    pub batch_size: usize,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            directories: Vec::new(),
            recursive: true,
            incremental: true,
            parallel: true,
            batch_size: 100,
        }
    }
}

pub struct LibraryScanner {
    database: LibraryDatabase,
    metadata_extractor: MetadataExtractor,
    config: ScanConfig,
    progress_sender: Option<Sender<ScanProgress>>,
    is_scanning: bool,
}

impl LibraryScanner {
    pub fn new(database: LibraryDatabase) -> Self {
        Self {
            database,
            metadata_extractor: MetadataExtractor::default(),
            config: ScanConfig::default(),
            progress_sender: None,
            is_scanning: false,
        }
    }

    pub fn with_config(mut self, config: ScanConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_progress_sender(mut self, sender: Sender<ScanProgress>) -> Self {
        self.progress_sender = Some(sender);
        self
    }

    pub fn is_scanning(&self) -> bool {
        self.is_scanning
    }

    pub fn scan(&mut self) -> Result<ScanResult> {
        if self.is_scanning {
            return Err(AudioError::InvalidState("Scan already in progress".into()));
        }

        self.is_scanning = true;
        let start_time = Instant::now();

        info!("Starting library scan of {} directories", self.config.directories.len());

        let mut all_files = Vec::new();
        for directory in &self.config.directories {
            let files = self.collect_audio_files(directory, self.config.recursive)?;
            all_files.extend(files);
        }

        let total_files = all_files.len();
        debug!("Found {} audio files", total_files);

        if let Some(ref sender) = self.progress_sender {
            let _ = sender.send(ScanProgress::Started { total_files });
        }

        let result = if self.config.incremental {
            self.scan_incremental(&all_files, start_time)
        } else {
            self.scan_full(&all_files, start_time)
        };

        self.is_scanning = false;
        result
    }

    fn collect_audio_files(&self, directory: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        if !directory.exists() {
            warn!("Directory does not exist: {}", directory.display());
            return Ok(files);
        }

        let entries = fs::read_dir(directory)
            .map_err(|e| AudioError::IoError(format!("Failed to read directory {}: {}", directory.display(), e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                AudioError::IoError(format!("Failed to read entry in {}: {}", directory.display(), e))
            })?;

            let path = entry.path();

            if path.is_dir() && recursive {
                let sub_files = self.collect_audio_files(&path, recursive)?;
                files.extend(sub_files);
            } else if path.is_file() && is_audio_file(&path) {
                files.push(path);
            }
        }

        Ok(files)
    }

    fn scan_incremental(&mut self, files: &[PathBuf], start_time: Instant) -> Result<ScanResult> {
        self.database.begin_transaction()?;

        let mut new_tracks = 0;
        let mut updated_tracks = 0;
        let mut failed_files = 0;
        let mut existing_paths = HashSet::new();

        for (index, file) in files.iter().enumerate() {
            if let Some(ref sender) = self.progress_sender {
                let _ = sender.send(ScanProgress::Progress {
                    current: index + 1,
                    total: files.len(),
                    file: file.clone(),
                });
            }

            match self.process_file_incremental(file, &mut existing_paths) {
                Ok(status) => {
                    match status {
                        FileStatus::New => new_tracks += 1,
                        FileStatus::Updated => updated_tracks += 1,
                        FileStatus::Unchanged => {}
                    }
                }
                Err(e) => {
                    failed_files += 1;
                    warn!("Failed to process file {}: {}", file.display(), e);
                    if let Some(ref sender) = self.progress_sender {
                        let _ = sender.send(ScanProgress::Error {
                            file: file.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }
        }

        let removed_tracks = self.remove_missing_files(&existing_paths)?;

        self.database.commit()?;

        let duration = start_time.elapsed();
        let stats = self.database.get_stats()?;

        let result = ScanResult {
            total_files_scanned: files.len(),
            new_tracks,
            updated_tracks,
            removed_tracks,
            failed_files,
            duration,
            stats,
        };

        if let Some(ref sender) = self.progress_sender {
            let _ = sender.send(ScanProgress::Completed {
                result: result.clone(),
            });
        }

        info!("Incremental scan completed: {} new, {} updated, {} removed in {:?}", 
            new_tracks, updated_tracks, removed_tracks, duration);

        Ok(result)
    }

    fn scan_full(&mut self, files: &[PathBuf], start_time: Instant) -> Result<ScanResult> {
        self.database.begin_transaction()?;

        let mut new_tracks = 0;
        let mut updated_tracks = 0;
        let mut failed_files = 0;

        for (index, file) in files.iter().enumerate() {
            if let Some(ref sender) = self.progress_sender {
                let _ = sender.send(ScanProgress::Progress {
                    current: index + 1,
                    total: files.len(),
                    file: file.clone(),
                });
            }

            match self.process_file_full(file) {
                Ok(status) => {
                    match status {
                        FileStatus::New => new_tracks += 1,
                        FileStatus::Updated => updated_tracks += 1,
                        FileStatus::Unchanged => {}
                    }
                }
                Err(e) => {
                    failed_files += 1;
                    warn!("Failed to process file {}: {}", file.display(), e);
                    if let Some(ref sender) = self.progress_sender {
                        let _ = sender.send(ScanProgress::Error {
                            file: file.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }
        }

        self.database.commit()?;

        let duration = start_time.elapsed();
        let stats = self.database.get_stats()?;

        let result = ScanResult {
            total_files_scanned: files.len(),
            new_tracks,
            updated_tracks,
            removed_tracks: 0,
            failed_files,
            duration,
            stats,
        };

        if let Some(ref sender) = self.progress_sender {
            let _ = sender.send(ScanProgress::Completed {
                result: result.clone(),
            });
        }

        info!("Full scan completed: {} new, {} updated in {:?}", 
            new_tracks, updated_tracks, duration);

        Ok(result)
    }

    fn process_file_incremental(&self, file: &Path, existing_paths: &mut HashSet<PathBuf>) -> Result<FileStatus> {
        let metadata = fs::metadata(file)
            .map_err(|e| AudioError::IoError(format!("Failed to get file metadata: {}", e)))?;

        let modified_time = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        existing_paths.insert(file.to_path_buf());

        if let Some(existing_track) = self.database.get_track_by_path(file)? {
            if existing_track.modified_time == modified_time {
                return Ok(FileStatus::Unchanged);
            }

            let track_metadata = self.metadata_extractor.extract(file)?;
            self.database.add_track(&track_metadata)?;
            debug!("Updated track: {}", file.display());
            Ok(FileStatus::Updated)
        } else {
            let track_metadata = self.metadata_extractor.extract(file)?;
            self.database.add_track(&track_metadata)?;
            debug!("Added new track: {}", file.display());
            Ok(FileStatus::New)
        }
    }

    fn process_file_full(&self, file: &Path) -> Result<FileStatus> {
        let track_metadata = self.metadata_extractor.extract(file)?;
        self.database.add_track(&track_metadata)?;
        debug!("Processed track: {}", file.display());
        Ok(FileStatus::New)
    }

    fn remove_missing_files(&self, existing_paths: &HashSet<PathBuf>) -> Result<usize> {
        let all_tracks = self.database.get_all_tracks(None, None)?;
        let mut removed_count = 0;

        for track in all_tracks {
            if !existing_paths.contains(&track.path) {
                if self.database.delete_track(&track.path)? {
                    debug!("Removed missing track: {}", track.path.display());
                    removed_count += 1;
                }
            }
        }

        Ok(removed_count)
    }

    pub fn scan_single_file(&self, file: &Path) -> Result<bool> {
        if !is_audio_file(file) {
            return Ok(false);
        }

        let track_metadata = self.metadata_extractor.extract(file)?;
        self.database.add_track(&track_metadata)?;
        info!("Added single file: {}", file.display());
        Ok(true)
    }

    pub fn remove_file(&self, file: &Path) -> Result<bool> {
        let removed = self.database.delete_track(file)?;
        if removed {
            info!("Removed file from library: {}", file.display());
        }
        Ok(removed)
    }

    pub fn refresh_file(&self, file: &Path) -> Result<bool> {
        if !is_audio_file(file) {
            return Ok(false);
        }

        let track_metadata = self.metadata_extractor.extract(file)?;
        self.database.add_track(&track_metadata)?;
        info!("Refreshed file in library: {}", file.display());
        Ok(true)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileStatus {
    New,
    Updated,
    Unchanged,
}

pub struct ScanProgressTracker {
    start_time: Instant,
    total_files: usize,
    processed_files: usize,
    new_tracks: usize,
    updated_tracks: usize,
    failed_files: usize,
}

impl ScanProgressTracker {
    pub fn new(total_files: usize) -> Self {
        Self {
            start_time: Instant::now(),
            total_files,
            processed_files: 0,
            new_tracks: 0,
            updated_tracks: 0,
            failed_files: 0,
        }
    }

    pub fn update(&mut self, status: FileStatus) {
        self.processed_files += 1;
        match status {
            FileStatus::New => self.new_tracks += 1,
            FileStatus::Updated => self.updated_tracks += 1,
            FileStatus::Unchanged => {}
        }
    }

    pub fn record_failure(&mut self) {
        self.processed_files += 1;
        self.failed_files += 1;
    }

    pub fn progress(&self) -> f32 {
        if self.total_files == 0 {
            1.0
        } else {
            self.processed_files as f32 / self.total_files as f32
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn estimated_remaining(&self) -> Option<Duration> {
        if self.processed_files == 0 {
            return None;
        }

        let elapsed = self.elapsed();
        let avg_time_per_file = elapsed / self.processed_files as u32;
        let remaining_files = self.total_files.saturating_sub(self.processed_files);
        Some(avg_time_per_file * remaining_files as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_progress_tracker() {
        let mut tracker = ScanProgressTracker::new(100);
        
        assert_eq!(tracker.progress(), 0.0);
        
        tracker.update(FileStatus::New);
        assert_eq!(tracker.progress(), 0.01);
        assert_eq!(tracker.new_tracks, 1);
        
        tracker.update(FileStatus::Updated);
        assert_eq!(tracker.progress(), 0.02);
        assert_eq!(tracker.updated_tracks, 1);
        
        tracker.record_failure();
        assert_eq!(tracker.progress(), 0.03);
        assert_eq!(tracker.failed_files, 1);
    }
}
