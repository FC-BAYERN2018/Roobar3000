use crate::audio::format::AudioFormat;
use crate::utils::error::{AudioError, Result};
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, Device, Host, Stream, StreamConfig, SampleFormat as CpalFormat};
use std::sync::Arc;
use parking_lot::Mutex;
use tracing::{info, debug, error, warn};

pub trait OutputBackend: Send + Sync {
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
    fn set_volume(&mut self, volume: f32) -> Result<()>;
    fn get_volume(&self) -> f32;
    fn is_playing(&self) -> bool;
    fn get_format(&self) -> Option<AudioFormat>;
}

pub struct AudioOutput {
    device: Device,
    stream: Option<Stream>,
    config: StreamConfig,
    format: AudioFormat,
    volume: Arc<Mutex<f32>>,
    is_playing: Arc<Mutex<bool>>,
}

impl AudioOutput {
    pub fn new(device: Device, format: AudioFormat) -> Result<Self> {
        let supported_configs = device.supported_output_configs()
            .map_err(|e| AudioError::OutputError(format!("Failed to get supported configs: {}", e)))?;

        let mut best_config = None;
        let mut best_score = -1i32;

        for config in supported_configs {
            let score = Self::score_config(&config, &format);
            if score > best_score {
                best_score = score;
                best_config = Some(config);
            }
        }

        let config = best_config.ok_or_else(|| {
            AudioError::OutputError("No suitable configuration found".into())
        })?;

        let config = config.with_sample_rate(cpal::SampleRate(format.sample_rate));
        let stream_config: StreamConfig = config.into();

        info!("AudioOutput created with config: {:?}", stream_config);

        Ok(Self {
            device,
            stream: None,
            config: stream_config,
            format,
            volume: Arc::new(Mutex::new(1.0)),
            is_playing: Arc::new(Mutex::new(false)),
        })
    }

    fn score_config(config: &cpal::SupportedStreamConfigRange, format: &AudioFormat) -> i32 {
        let mut score = 0;

        if config.channels() == format.channels as u16 {
            score += 100;
        } else {
            score -= (config.channels() as i32 - format.channels as i32).abs() * 10;
        }

        if config.min_sample_rate().0 <= format.sample_rate && 
           config.max_sample_rate().0 >= format.sample_rate {
            score += 50;
        }

        if config.sample_format() == Self::to_cpal_format(format.sample_format) {
            score += 30;
        }

        score
    }

    fn to_cpal_format(format: crate::audio::format::SampleFormat) -> CpalFormat {
        match format {
            crate::audio::format::SampleFormat::U8 => CpalFormat::U8,
            crate::audio::format::SampleFormat::S16 => CpalFormat::I16,
            crate::audio::format::SampleFormat::S24 => CpalFormat::I32,
            crate::audio::format::SampleFormat::S32 => CpalFormat::I32,
            crate::audio::format::SampleFormat::F32 => CpalFormat::F32,
            crate::audio::format::SampleFormat::F64 => CpalFormat::F64,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        if self.stream.is_some() {
            return Ok(());
        }

        let volume = Arc::clone(&self.volume);
        let is_playing = Arc::clone(&self.is_playing);
        let sample_format = self.config.sample_format();

        let stream = match sample_format {
            CpalFormat::F32 => self.create_stream::<f32>(volume, is_playing)?,
            CpalFormat::I16 => self.create_stream::<i16>(volume, is_playing)?,
            CpalFormat::U8 => self.create_stream::<u8>(volume, is_playing)?,
            _ => return Err(AudioError::OutputError("Unsupported sample format".into())),
        };

        self.stream = Some(stream);
        *is_playing.lock() = true;
        info!("AudioOutput started");
        Ok(())
    }

    fn create_stream<T>(&self, volume: Arc<Mutex<f32>>, is_playing: Arc<Mutex<bool>>) -> Result<Stream>
    where
        T: cpal::Sample + cpal::SizedSample + Send + 'static,
    {
        let err_fn = |err| error!("Audio output error: {}", err);

        let stream = self.device.build_output_stream(
            &self.config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let vol = *volume.lock();
                if vol > 0.0 {
                    for sample in data.iter_mut() {
                        let val = sample.to_f32();
                        *sample = T::from_sample::<f32>(val * vol);
                    }
                } else {
                    data.fill(T::MID);
                }
            },
            err_fn,
            None,
        ).map_err(|e| AudioError::OutputError(format!("Failed to build output stream: {}", e)))?;

        Ok(stream)
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        *self.is_playing.lock() = false;
        info!("AudioOutput stopped");
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        if let Some(stream) = &self.stream {
            stream.pause().map_err(|e| {
                AudioError::OutputError(format!("Failed to pause stream: {}", e))
            })?;
        }
        *self.is_playing.lock() = false;
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
        if let Some(stream) = &self.stream {
            stream.resume().map_err(|e| {
                AudioError::OutputError(format!("Failed to resume stream: {}", e))
            })?;
        }
        *self.is_playing.lock() = true;
        Ok(())
    }

    pub fn set_volume(&mut self, volume: f32) -> Result<()> {
        let vol = volume.clamp(0.0, 1.0);
        *self.volume.lock() = vol;
        debug!("Volume set to {:.2}", vol);
        Ok(())
    }

    pub fn get_volume(&self) -> f32 {
        *self.volume.lock()
    }

    pub fn is_playing(&self) -> bool {
        *self.is_playing.lock()
    }

    pub fn get_format(&self) -> Option<AudioFormat> {
        Some(self.format)
    }

    pub fn device_name(&self) -> Result<String> {
        self.device.name().map_err(|e| {
            AudioError::OutputError(format!("Failed to get device name: {}", e))
        })
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
