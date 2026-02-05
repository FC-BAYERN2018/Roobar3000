use crate::utils::error::{AudioError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub buffer_size_frames: usize,
    pub buffer_pool_size: usize,
    pub ring_buffer_size: usize,
    pub target_buffer_level: f32,
    pub default_sample_rate: u32,
    pub default_channels: u16,
    pub default_bit_depth: u8,
    pub bitperfect: BitPerfectConfig,
    pub dsp: DSPConfig,
    pub output: OutputConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitPerfectConfig {
    pub enabled: bool,
    pub prefer_integer: bool,
    pub auto_sample_rate: bool,
    pub allow_resampling: bool,
    pub exclusive_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DSPConfig {
    pub enabled: bool,
    pub resampler_quality: String,
    pub eq_enabled: bool,
    pub eq_bands: Vec<EQBandConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EQBandConfig {
    pub frequency: f32,
    pub gain_db: f32,
    pub q: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub device_name: Option<String>,
    pub volume: f32,
    pub latency_ms: u32,
    pub exclusive_mode: bool,
}

impl Default for BitPerfectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prefer_integer: true,
            auto_sample_rate: true,
            allow_resampling: false,
            exclusive_mode: false,
        }
    }
}

impl Default for DSPConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            resampler_quality: "high".to_string(),
            eq_enabled: false,
            eq_bands: vec![],
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            volume: 1.0,
            latency_ms: 50,
            exclusive_mode: false,
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            buffer_size_frames: 4096,
            buffer_pool_size: 16,
            ring_buffer_size: 65536,
            target_buffer_level: 0.5,
            default_sample_rate: 44100,
            default_channels: 2,
            default_bit_depth: 16,
            bitperfect: BitPerfectConfig::default(),
            dsp: DSPConfig::default(),
            output: OutputConfig::default(),
        }
    }
}

impl AudioConfig {
    pub fn validate(&self) -> Result<()> {
        if self.buffer_size_frames == 0 {
            return Err(AudioError::InvalidParameter("Buffer size frames cannot be 0".into()));
        }

        if self.buffer_pool_size == 0 {
            return Err(AudioError::InvalidParameter("Buffer pool size cannot be 0".into()));
        }

        if self.ring_buffer_size == 0 {
            return Err(AudioError::InvalidParameter("Ring buffer size cannot be 0".into()));
        }

        if self.target_buffer_level <= 0.0 || self.target_buffer_level > 1.0 {
            return Err(AudioError::InvalidParameter("Target buffer level must be between 0 and 1".into()));
        }

        if self.default_sample_rate == 0 {
            return Err(AudioError::InvalidParameter("Default sample rate cannot be 0".into()));
        }

        if self.default_channels == 0 {
            return Err(AudioError::InvalidParameter("Default channels cannot be 0".into()));
        }

        if self.output.volume < 0.0 || self.output.volume > 2.0 {
            return Err(AudioError::InvalidParameter("Volume must be between 0 and 2".into()));
        }

        Ok(())
    }

    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size_frames = size;
        self
    }

    pub fn with_sample_rate(mut self, rate: u32) -> Self {
        self.default_sample_rate = rate;
        self
    }

    pub fn with_channels(mut self, channels: u16) -> Self {
        self.default_channels = channels;
        self
    }

    pub fn with_bitperfect(mut self, enabled: bool) -> Self {
        self.bitperfect.enabled = enabled;
        self
    }

    pub fn with_dsp(mut self, enabled: bool) -> Self {
        self.dsp.enabled = enabled;
        self
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.output.volume = volume.clamp(0.0, 2.0);
        self
    }
}

impl EQBandConfig {
    pub fn new(frequency: f32, gain_db: f32, q: f32) -> Self {
        Self {
            frequency,
            gain_db,
            q,
        }
    }
}
