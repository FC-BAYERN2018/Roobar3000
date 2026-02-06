pub mod models;
pub mod metadata;
pub mod database;
pub mod scanner;
pub mod watch;

pub use models::{Track, Album, Artist, Playlist, LibraryStats};
pub use metadata::{MetadataExtractor, TrackMetadata};
pub use database::{LibraryDatabase, DatabaseConfig};
pub use scanner::{LibraryScanner, ScanProgress, ScanResult};
pub use watch::{LibraryWatcher, WatchEvent};
