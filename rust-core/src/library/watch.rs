use crate::library::database::LibraryDatabase;
use crate::library::metadata::is_audio_file;
use crate::library::scanner::LibraryScanner;
use crate::utils::error::{AudioError, Result};
use crossbeam_channel::{Sender, Receiver, unbounded};
use notify::{Watcher, RecursiveMode, Event, EventKind, RecommendedWatcher};
use notify::event::ModifyKind;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{info, debug, warn};

#[derive(Debug, Clone)]
pub enum WatchEvent {
    FileAdded { path: PathBuf },
    FileRemoved { path: PathBuf },
    FileModified { path: PathBuf },
    FileRenamed { old_path: PathBuf, new_path: PathBuf },
    Error { error: String },
}

#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub debounce_delay_ms: u64,
    pub batch_size: usize,
    pub auto_scan: bool,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            debounce_delay_ms: 1000,
            batch_size: 10,
            auto_scan: true,
        }
    }
}

pub struct LibraryWatcher {
    database: Arc<Mutex<LibraryDatabase>>,
    scanner: Arc<Mutex<LibraryScanner>>,
    watcher: Option<RecommendedWatcher>,
    event_sender: Sender<WatchEvent>,
    event_receiver: Receiver<WatchEvent>,
    config: WatchConfig,
    watched_paths: HashMap<PathBuf, RecursiveMode>,
    is_running: bool,
    worker_handle: Option<JoinHandle<()>>,
}

impl LibraryWatcher {
    pub fn new(database: LibraryDatabase, scanner: LibraryScanner) -> Result<Self> {
        let (event_sender, event_receiver) = unbounded();

        Ok(Self {
            database: Arc::new(Mutex::new(database)),
            scanner: Arc::new(Mutex::new(scanner)),
            watcher: None,
            event_sender,
            event_receiver,
            config: WatchConfig::default(),
            watched_paths: HashMap::new(),
            is_running: false,
            worker_handle: None,
        })
    }

    pub fn with_config(mut self, config: WatchConfig) -> Self {
        self.config = config;
        self
    }

    pub fn event_sender(&self) -> &Sender<WatchEvent> {
        &self.event_sender
    }

    pub fn event_receiver(&self) -> &Receiver<WatchEvent> {
        &self.event_receiver
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn watch(&mut self, path: &Path, recursive: bool) -> Result<()> {
        if !path.exists() {
            return Err(AudioError::NotFound(format!("Path does not exist: {}", path.display())));
        }

        let mode = if recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        if let Some(ref mut watcher) = self.watcher {
            watcher.watch(path, mode)
                .map_err(|e| AudioError::IoError(format!("Failed to watch path: {}", e)))?;
            
            self.watched_paths.insert(path.to_path_buf(), mode);
            info!("Watching path: {} (recursive: {})", path.display(), recursive);
        } else {
            return Err(AudioError::InvalidState("Watcher not started".into()));
        }

        Ok(())
    }

    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        if let Some(ref mut watcher) = self.watcher {
            watcher.unwatch(path)
                .map_err(|e| AudioError::IoError(format!("Failed to unwatch path: {}", e)))?;
            
            self.watched_paths.remove(path);
            info!("Unwatched path: {}", path.display());
        }

        Ok(())
    }

    pub fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }

        let event_sender = self.event_sender.clone();
        let database = Arc::clone(&self.database);
        let scanner = Arc::clone(&self.scanner);
        let config = self.config.clone();

        let watcher = notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if let Some(path) = event.paths.first() {
                        if is_audio_file(path) {
                            let watch_event = Self::convert_event(&event, path);
                            if let Some(evt) = watch_event {
                                let _ = event_sender.send(evt);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Watch error: {}", e);
                    let _ = event_sender.send(WatchEvent::Error {
                        error: e.to_string(),
                    });
                }
            }
        }).map_err(|e| AudioError::IoError(format!("Failed to create watcher: {}", e)))?;

        self.watcher = Some(watcher);

        let receiver = self.event_receiver.clone();
        let db = Arc::clone(&database);
        let scan = Arc::clone(&scanner);
        let cfg = config.clone();

        let handle = thread::spawn(move || {
            event_worker(receiver, db, scan, cfg);
        });

        self.worker_handle = Some(handle);
        self.is_running = true;

        info!("Library watcher started");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            return Ok(());
        }

        self.watcher = None;
        self.is_running = false;

        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }

        info!("Library watcher stopped");
        Ok(())
    }

    fn convert_event(event: &Event, path: &Path) -> Option<WatchEvent> {
        match event.kind {
            EventKind::Create(_) => {
                debug!("File created: {}", path.display());
                Some(WatchEvent::FileAdded {
                    path: path.to_path_buf(),
                })
            }
            EventKind::Remove(_) => {
                debug!("File removed: {}", path.display());
                Some(WatchEvent::FileRemoved {
                    path: path.to_path_buf(),
                })
            }
            EventKind::Modify(ModifyKind::Data(_)) | EventKind::Modify(ModifyKind::Metadata(_)) => {
                debug!("File modified: {}", path.display());
                Some(WatchEvent::FileModified {
                    path: path.to_path_buf(),
                })
            }
            EventKind::Modify(ModifyKind::Name(_)) => {
                if let Some(new_path) = event.paths.get(1) {
                    debug!("File renamed: {} -> {}", path.display(), new_path.display());
                    Some(WatchEvent::FileRenamed {
                        old_path: path.to_path_buf(),
                        new_path: new_path.to_path_buf(),
                    })
                } else {
                    Some(WatchEvent::FileRemoved {
                        path: path.to_path_buf(),
                    })
                }
            }
            _ => None,
        }
    }

    pub fn watched_paths(&self) -> Vec<PathBuf> {
        self.watched_paths.keys().cloned().collect()
    }

    pub fn is_watching(&self, path: &Path) -> bool {
        self.watched_paths.contains_key(path)
    }
}

impl Drop for LibraryWatcher {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn event_worker(
    receiver: Receiver<WatchEvent>,
    database: Arc<Mutex<LibraryDatabase>>,
    scanner: Arc<Mutex<LibraryScanner>>,
    config: WatchConfig,
) {
    let mut event_queue: Vec<WatchEvent> = Vec::new();
    let mut last_process = std::time::Instant::now();
    let debounce_delay = Duration::from_millis(config.debounce_delay_ms);

    loop {
        let event = match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(evt) => evt,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                if !event_queue.is_empty() && last_process.elapsed() >= debounce_delay {
                    process_event_queue(&event_queue, &database, &scanner, &config);
                    event_queue.clear();
                    last_process = std::time::Instant::now();
                }
                continue;
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                info!("Event worker disconnected");
                break;
            }
        };

        event_queue.push(event);

        if event_queue.len() >= config.batch_size || last_process.elapsed() >= debounce_delay {
            process_event_queue(&event_queue, &database, &scanner, &config);
            event_queue.clear();
            last_process = std::time::Instant::now();
        }
    }
}

fn process_event_queue(
    events: &[WatchEvent],
    _database: &Arc<Mutex<LibraryDatabase>>,
    scanner: &Arc<Mutex<LibraryScanner>>,
    config: &WatchConfig,
) {
    if events.is_empty() {
        return;
    }

    info!("Processing {} watch events", events.len());

    let scan = scanner.lock();

    for event in events {
        match event {
            WatchEvent::FileAdded { path } => {
                if config.auto_scan {
                    if let Err(e) = scan.scan_single_file(path) {
                        warn!("Failed to scan added file {}: {}", path.display(), e);
                    }
                }
            }
            WatchEvent::FileRemoved { path } => {
                if let Err(e) = scan.remove_file(path) {
                    warn!("Failed to remove file {}: {}", path.display(), e);
                }
            }
            WatchEvent::FileModified { path } => {
                if config.auto_scan {
                    if let Err(e) = scan.refresh_file(path) {
                        warn!("Failed to refresh modified file {}: {}", path.display(), e);
                    }
                }
            }
            WatchEvent::FileRenamed { old_path, new_path } => {
                if let Err(e) = scan.remove_file(old_path) {
                    warn!("Failed to remove renamed file {}: {}", old_path.display(), e);
                }
                if config.auto_scan {
                    if let Err(e) = scan.scan_single_file(new_path) {
                        warn!("Failed to scan renamed file {}: {}", new_path.display(), e);
                    }
                }
            }
            WatchEvent::Error { error } => {
                warn!("Watch error: {}", error);
            }
        }
    }

    info!("Processed {} watch events", events.len());
}

#[derive(Debug, Clone)]
pub struct WatchStats {
    pub events_processed: u64,
    pub files_added: u64,
    pub files_removed: u64,
    pub files_modified: u64,
    pub files_renamed: u64,
    pub errors: u64,
    pub uptime: Duration,
}

pub struct WatchStatsCollector {
    events_processed: u64,
    files_added: u64,
    files_removed: u64,
    files_modified: u64,
    files_renamed: u64,
    errors: u64,
    start_time: std::time::Instant,
}

impl WatchStatsCollector {
    pub fn new() -> Self {
        Self {
            events_processed: 0,
            files_added: 0,
            files_removed: 0,
            files_modified: 0,
            files_renamed: 0,
            errors: 0,
            start_time: std::time::Instant::now(),
        }
    }

    pub fn record_event(&mut self, event: &WatchEvent) {
        self.events_processed += 1;
        match event {
            WatchEvent::FileAdded { .. } => self.files_added += 1,
            WatchEvent::FileRemoved { .. } => self.files_removed += 1,
            WatchEvent::FileModified { .. } => self.files_modified += 1,
            WatchEvent::FileRenamed { .. } => self.files_renamed += 1,
            WatchEvent::Error { .. } => self.errors += 1,
        }
    }

    pub fn get_stats(&self) -> WatchStats {
        WatchStats {
            events_processed: self.events_processed,
            files_added: self.files_added,
            files_removed: self.files_removed,
            files_modified: self.files_modified,
            files_renamed: self.files_renamed,
            errors: self.errors,
            uptime: self.start_time.elapsed(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for WatchStatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_stats_collector() {
        let mut collector = WatchStatsCollector::new();
        
        collector.record_event(&WatchEvent::FileAdded {
            path: PathBuf::from("test.mp3"),
        });
        collector.record_event(&WatchEvent::FileRemoved {
            path: PathBuf::from("test.mp3"),
        });
        
        let stats = collector.get_stats();
        assert_eq!(stats.events_processed, 2);
        assert_eq!(stats.files_added, 1);
        assert_eq!(stats.files_removed, 1);
    }
}
