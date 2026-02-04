use crate::config::audio::AudioConfig;
use crate::utils::error::{AudioError, Result};
use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub audio: AudioConfig,
    pub library: LibraryConfig,
    pub ipc: IPCConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryConfig {
    pub scan_directories: Vec<String>,
    pub auto_scan: bool,
    pub scan_interval_seconds: u64,
    pub watch_changes: bool,
    pub database_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IPCConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub port: u16,
    pub max_connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file_enabled: bool,
    pub file_path: String,
    pub max_file_size_mb: u64,
    pub max_files: u32,
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self {
            scan_directories: vec![],
            auto_scan: false,
            scan_interval_seconds: 3600,
            watch_changes: true,
            database_path: "library.db".to_string(),
        }
    }
}

impl Default for IPCConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            max_connections: 10,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file_enabled: false,
            file_path: "roobar3000.log".to_string(),
            max_file_size_mb: 10,
            max_files: 5,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            audio: AudioConfig::default(),
            library: LibraryConfig::default(),
            ipc: IPCConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

pub struct ConfigManager {
    config_path: Option<PathBuf>,
    config: AppConfig,
}

impl ConfigManager {
    pub fn new(config_path: Option<String>) -> Result<Self> {
        let config_path = config_path.map(PathBuf::from);
        
        let config = if let Some(ref path) = config_path {
            Self::load_from_file(path)?
        } else {
            AppConfig::default()
        };

        info!("ConfigManager initialized");
        Ok(Self { config_path, config })
    }

    pub fn load(&self) -> Result<AppConfig> {
        Ok(self.config.clone())
    }

    pub fn load_from_file(path: &Path) -> Result<AppConfig> {
        let config = Config::builder()
            .add_source(File::from(path))
            .add_source(Environment::with_prefix("ROOBAR"))
            .build()?;

        let app_config: AppConfig = config.try_deserialize()?;
        
        info!("Loaded config from: {}", path.display());
        debug!("Config: {:?}", app_config);
        
        Ok(app_config)
    }

    pub fn save(&self) -> Result<()> {
        if let Some(path) = &self.config_path {
            self.save_to_file(path)?;
        } else {
            warn!("No config path set, cannot save");
        }
        Ok(())
    }

    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let toml = toml::to_string_pretty(&self.config)
            .map_err(|e| AudioError::IoError(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(path, toml)?;
        
        info!("Saved config to: {}", path.display());
        Ok(())
    }

    pub fn update(&mut self, config: AppConfig) {
        self.config = config;
    }

    pub fn update_audio(&mut self, audio_config: AudioConfig) {
        self.config.audio = audio_config;
    }

    pub fn get_audio(&self) -> &AudioConfig {
        &self.config.audio
    }

    pub fn get_library(&self) -> &LibraryConfig {
        &self.config.library
    }

    pub fn get_ipc(&self) -> &IPCConfig {
        &self.config.ipc
    }

    pub fn get_logging(&self) -> &LoggingConfig {
        &self.config.logging
    }

    pub fn reload(&mut self) -> Result<()> {
        if let Some(path) = &self.config_path {
            self.config = Self::load_from_file(path)?;
            info!("Config reloaded from: {}", path.display());
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        self.config.audio.validate()?;
        
        if self.config.ipc.port == 0 {
            return Err(AudioError::InvalidParameter("IPC port cannot be 0".into()));
        }

        if self.config.ipc.max_connections == 0 {
            return Err(AudioError::InvalidParameter("Max connections cannot be 0".into()));
        }

        Ok(())
    }
}
