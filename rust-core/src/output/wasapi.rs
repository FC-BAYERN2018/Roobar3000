use crate::audio::format::AudioFormat;
use crate::output::backend::OutputBackend;
use crate::utils::error::{AudioError, Result};
use cpal::{traits::DeviceTrait, Device, Stream, StreamConfig};
use std::sync::Arc;
use parking_lot::Mutex;
use tracing::{info, debug, error};

#[cfg(target_os = "windows")]
pub struct WasapiOutput {
    device: Device,
    stream: Option<Stream>,
    config: StreamConfig,
    format: AudioFormat,
    volume: Arc<Mutex<f32>>,
    is_playing: Arc<Mutex<bool>>,
    exclusive_mode: bool,
}

#[cfg(target_os = "windows")]
impl WasapiOutput {
    pub fn new_exclusive(device: Device, format: AudioFormat) -> Result<Self> {
        let config = StreamConfig {
            channels: format.channels,
            sample_rate: cpal::SampleRate(format.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(512),
        };

        info!("WASAPI Exclusive mode created: format={}, exclusive=true", format);

        Ok(Self {
            device,
            stream: None,
            config,
            format,
            volume: Arc::new(Mutex::new(1.0)),
            is_playing: Arc::new(Mutex::new(false)),
            exclusive_mode: true,
        })
    }

    pub fn new_shared(device: Device, format: AudioFormat) -> Result<Self> {
        let config = StreamConfig {
            channels: format.channels,
            sample_rate: cpal::SampleRate(format.sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        info!("WASAPI Shared mode created: format={}, exclusive=false", format);

        Ok(Self {
            device,
            stream: None,
            config,
            format,
            volume: Arc::new(Mutex::new(1.0)),
            is_playing: Arc::new(Mutex::new(false)),
            exclusive_mode: false,
        })
    }

    pub fn is_exclusive(&self) -> bool {
        self.exclusive_mode
    }

    fn create_stream(&self, volume: Arc<Mutex<f32>>, _is_playing: Arc<Mutex<bool>>) -> Result<Stream>
    {
        let err_fn = |err| error!("WASAPI output error: {}", err);

        let stream = self.device.build_output_stream(
            &self.config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let vol = *volume.lock();
                if vol > 0.0 {
                    for sample in data.iter_mut() {
                        *sample *= vol;
                    }
                } else {
                    data.fill(0.0);
                }
            },
            err_fn,
            None,
        ).map_err(|e| AudioError::OutputError(format!("Failed to build WASAPI stream: {}", e)))?;

        Ok(stream)
    }

    pub fn device_name(&self) -> Result<String> {
        self.device.name().map_err(|e| {
            AudioError::OutputError(format!("Failed to get device name: {}", e))
        })
    }
}

#[cfg(target_os = "windows")]
impl OutputBackend for WasapiOutput {
    fn start(&mut self) -> Result<()> {
        if self.stream.is_some() {
            return Ok(());
        }

        let volume = Arc::clone(&self.volume);
        let is_playing = Arc::clone(&self.is_playing);

        let stream = self.create_stream(volume, is_playing)?;

        self.stream = Some(stream);
        *self.is_playing.lock() = true;
        info!("WASAPI output started (exclusive: {})", self.exclusive_mode);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        *self.is_playing.lock() = false;
        info!("WASAPI output stopped");
        Ok(())
    }

    fn pause(&mut self) {
        *self.is_playing.lock() = false;
    }

    fn resume(&mut self) {
        *self.is_playing.lock() = true;
    }

    fn set_volume(&mut self, volume: f32) -> Result<()> {
        let vol = volume.clamp(0.0, 1.0);
        *self.volume.lock() = vol;
        debug!("WASAPI volume set to {:.2}", vol);
        Ok(())
    }

    fn get_volume(&self) -> f32 {
        *self.volume.lock()
    }

    fn is_playing(&self) -> bool {
        *self.is_playing.lock()
    }

    fn get_format(&self) -> Option<AudioFormat> {
        Some(self.format)
    }
}

#[cfg(target_os = "windows")]
impl Drop for WasapiOutput {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(not(target_os = "windows"))]
pub struct WasapiOutput;

#[cfg(not(target_os = "windows"))]
impl WasapiOutput {
    pub fn new_exclusive(_device: Device, _format: AudioFormat) -> Result<Self> {
        Err(AudioError::OutputError("WASAPI is only available on Windows".into()))
    }

    pub fn new_shared(_device: Device, _format: AudioFormat) -> Result<Self> {
        Err(AudioError::OutputError("WASAPI is only available on Windows".into()))
    }
}
