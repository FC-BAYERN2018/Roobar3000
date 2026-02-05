use crate::audio::format::AudioFormat;
use crate::output::device::{DeviceManager, DeviceInfo};
use crate::utils::error::{AudioError, Result};
use tracing::{info, debug, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitPerfectMode {
    Disabled,
    Automatic,
    Exclusive,
    Passthrough,
}

#[derive(Debug, Clone)]
pub struct BitPerfectConfig {
    pub mode: BitPerfectMode,
    pub prefer_integer: bool,
    pub auto_sample_rate: bool,
    pub allow_resampling: bool,
}

impl Default for BitPerfectConfig {
    fn default() -> Self {
        Self {
            mode: BitPerfectMode::Automatic,
            prefer_integer: true,
            auto_sample_rate: true,
            allow_resampling: false,
        }
    }
}

pub struct BitPerfectManager {
    config: BitPerfectConfig,
    device_manager: DeviceManager,
    current_format: Option<AudioFormat>,
    is_bitperfect: bool,
}

impl BitPerfectManager {
    pub fn new(config: BitPerfectConfig) -> Result<Self> {
        let device_manager = DeviceManager::new()?;
        
        info!("BitPerfectManager created with mode: {:?}", config.mode);
        
        Ok(Self {
            config,
            device_manager,
            current_format: None,
            is_bitperfect: false,
        })
    }

    pub fn set_config(&mut self, config: BitPerfectConfig) {
        info!("BitPerfect config updated: {:?}", config.mode);
        self.config = config;
    }

    pub fn config(&self) -> &BitPerfectConfig {
        &self.config
    }

    pub fn is_bitperfect(&self) -> bool {
        self.is_bitperfect
    }

    pub fn current_format(&self) -> Option<AudioFormat> {
        self.current_format
    }

    pub fn validate_format(&self, format: &AudioFormat) -> Result<bool> {
        if self.config.mode == BitPerfectMode::Disabled {
            return Ok(false);
        }

        if let Some(device) = self.device_manager.default_device() {
            if device.supports_format(format) {
                if self.config.prefer_integer && format.sample_format.is_integer() {
                    return Ok(true);
                }
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn find_best_device(&self, format: &AudioFormat) -> Option<usize> {
        if self.config.mode == BitPerfectMode::Disabled {
            return self.device_manager.default_device().map(|_| 0);
        }

        if let Some(device) = self.device_manager.find_best_device_for_format(format) {
            let index = device.info().index;
            debug!("Found best device for format {}: device {}", format, index);
            Some(index)
        } else {
            warn!("No suitable device found for format {}", format);
            self.device_manager.default_device().map(|d| d.info().index)
        }
    }

    pub fn prepare_format(&mut self, source_format: AudioFormat) -> Result<AudioFormat> {
        self.current_format = Some(source_format);
        self.is_bitperfect = self.validate_format(&source_format)?;

        if self.is_bitperfect {
            info!("Bit-Perfect mode active: {}", source_format);
            Ok(source_format)
        } else {
            if !self.config.allow_resampling {
                warn!("Bit-Perfect not available, resampling disabled");
                return Err(AudioError::BitPerfectError("Cannot achieve bit-perfect output".into()));
            }

            if let Some(device) = self.device_manager.default_device() {
                let target_rate = device.info().max_sample_rate;
                let target_format = AudioFormat::new(
                    target_rate,
                    source_format.channels,
                    source_format.sample_format,
                );

                warn!("Resampling required: {} Hz -> {} Hz", source_format.sample_rate, target_rate);
                Ok(target_format)
            } else {
                Ok(source_format)
            }
        }
    }

    pub fn check_integrity(&self) -> Result<bool> {
        if let Some(format) = self.current_format {
            if let Some(device) = self.device_manager.default_device() {
                let supported = device.supports_format(&format);
                if supported {
                    info!("Bit-Perfect integrity check passed");
                    return Ok(true);
                }
            }
        }
        
        warn!("Bit-Perfect integrity check failed");
        Ok(false)
    }

    pub fn get_device_manager(&self) -> &DeviceManager {
        &self.device_manager
    }

    pub fn get_device_manager_mut(&mut self) -> &mut DeviceManager {
        &mut self.device_manager
    }

    pub fn refresh_devices(&mut self) -> Result<()> {
        self.device_manager.refresh_devices()
    }

    pub fn get_diagnostics(&self) -> BitPerfectDiagnostics {
        BitPerfectDiagnostics {
            mode: self.config.mode,
            is_active: self.is_bitperfect,
            current_format: self.current_format,
            device_count: self.device_manager.devices().len(),
            default_device: self.device_manager.default_device().map(|d| d.info().clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BitPerfectDiagnostics {
    pub mode: BitPerfectMode,
    pub is_active: bool,
    pub current_format: Option<AudioFormat>,
    pub device_count: usize,
    pub default_device: Option<DeviceInfo>,
}

impl Default for BitPerfectManager {
    fn default() -> Self {
        Self::new(BitPerfectConfig::default()).expect("Failed to create BitPerfectManager")
    }
}
