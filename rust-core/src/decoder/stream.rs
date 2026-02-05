use crate::audio::format::AudioFormat;
use std::path::PathBuf;
use std::time::Duration;

pub struct AudioStream {
    path: PathBuf,
    format: AudioFormat,
    duration: Option<Duration>,
    position: Duration,
}

impl AudioStream {
    pub fn new(path: PathBuf, format: AudioFormat, duration: Option<Duration>) -> Self {
        Self {
            path,
            format,
            duration,
            position: Duration::ZERO,
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn format(&self) -> AudioFormat {
        self.format
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    pub fn position(&self) -> Duration {
        self.position
    }

    pub fn set_position(&mut self, pos: Duration) {
        self.position = pos;
    }

    pub fn progress(&self) -> f32 {
        self.duration.map_or(0.0, |d| {
            if d.as_secs_f64() > 0.0 {
                (self.position.as_secs_f64() / d.as_secs_f64()) as f32
            } else {
                0.0
            }
        })
    }

    pub fn is_complete(&self) -> bool {
        self.duration.map_or(false, |d| self.position >= d)
    }
}

pub struct StreamInfo {
    pub path: PathBuf,
    pub format: AudioFormat,
    pub duration: Option<Duration>,
    pub bitrate: Option<u32>,
    pub codec: String,
}

impl StreamInfo {
    pub fn new(path: PathBuf, format: AudioFormat, duration: Option<Duration>) -> Self {
        Self {
            path,
            format,
            duration,
            bitrate: None,
            codec: "Unknown".into(),
        }
    }

    pub fn with_bitrate(mut self, bitrate: u32) -> Self {
        self.bitrate = Some(bitrate);
        self
    }

    pub fn with_codec(mut self, codec: String) -> Self {
        self.codec = codec;
        self
    }
}
