use crate::audio::format::AudioFormat;
use crate::utils::error::{AudioError, Result};
use cpal::{traits::{DeviceTrait, HostTrait}, Device, Host, SupportedStreamConfigRange};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use tracing::{info, debug};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub name: String,
    pub index: usize,
    pub is_default: bool,
    pub supported_formats: Vec<String>,
    pub max_channels: u16,
    pub max_sample_rate: u32,
}

impl DeviceInfo {
    pub fn from_device(device: &Device, index: usize, is_default: bool) -> Result<Self> {
        let name = device.name().map_err(|e| {
            AudioError::OutputError(format!("Failed to get device name: {}", e))
        })?;

        let supported_configs = device.supported_output_configs()
            .map_err(|e| AudioError::OutputError(format!("Failed to get configs: {}", e)))?;

        let mut max_channels = 0;
        let mut max_sample_rate = 0;
        let mut supported_formats = Vec::new();

        for config in supported_configs {
            max_channels = max_channels.max(config.channels());
            max_sample_rate = max_sample_rate.max(config.max_sample_rate().0);
            supported_formats.push(format!("{:?}", config.sample_format()));
        }

        Ok(Self {
            name,
            index,
            is_default,
            supported_formats,
            max_channels,
            max_sample_rate,
        })
    }
}

pub struct AudioDevice {
    device: Device,
    info: DeviceInfo,
}

impl AudioDevice {
    pub fn new(device: Device, index: usize, is_default: bool) -> Result<Self> {
        let info = DeviceInfo::from_device(&device, index, is_default)?;
        Ok(Self { device, info })
    }

    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn supports_format(&self, format: &AudioFormat) -> bool {
        if let Ok(configs) = self.device.supported_output_configs() {
            for config in configs {
                if config.channels() == format.channels as u16 {
                    if config.min_sample_rate().0 <= format.sample_rate && 
                       config.max_sample_rate().0 >= format.sample_rate {
                        return true;
                    }
                }
            }
        }
        false
    }
}

pub struct DeviceManager {
    host: Host,
    devices: Vec<AudioDevice>,
    default_device: Option<usize>,
}

impl DeviceManager {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        info!("Using host: {:?}", host.id());

        let mut manager = Self {
            host,
            devices: Vec::new(),
            default_device: None,
        };

        manager.refresh_devices()?;
        Ok(manager)
    }

    pub fn refresh_devices(&mut self) -> Result<()> {
        self.devices.clear();
        self.default_device = None;

        let devices = self.host.output_devices()
            .map_err(|e| AudioError::OutputError(format!("Failed to get devices: {}", e)))?;

        let default_device = self.host.default_output_device()
            .map_err(|e| AudioError::OutputError(format!("Failed to get default device: {}", e)))?;

        let default_name = default_device.name().unwrap_or_default();

        for (index, device) in devices.enumerate() {
            let name = device.name().unwrap_or_default();
            let is_default = name == default_name;

            if let Ok(audio_device) = AudioDevice::new(device, index, is_default) {
                if is_default {
                    self.default_device = Some(self.devices.len());
                }
                self.devices.push(audio_device);
                debug!("Found device: {} (default: {})", name, is_default);
            }
        }

        info!("Found {} audio devices", self.devices.len());
        Ok(())
    }

    pub fn devices(&self) -> &[AudioDevice] {
        &self.devices
    }

    pub fn device_infos(&self) -> Vec<DeviceInfo> {
        self.devices.iter().map(|d| d.info().clone()).collect()
    }

    pub fn default_device(&self) -> Option<&AudioDevice> {
        self.default_device.and_then(|idx| self.devices.get(idx))
    }

    pub fn get_device(&self, index: usize) -> Option<&AudioDevice> {
        self.devices.get(index)
    }

    pub fn find_best_device_for_format(&self, format: &AudioFormat) -> Option<&AudioDevice> {
        self.devices.iter().find(|d| d.supports_format(format))
    }

    pub fn get_device_by_name(&self, name: &str) -> Option<&AudioDevice> {
        self.devices.iter().find(|d| d.info().name == name)
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new().expect("Failed to create DeviceManager")
    }
}
