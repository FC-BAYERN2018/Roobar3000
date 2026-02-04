use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleFormat {
    U8,
    S16,
    S24,
    S32,
    F32,
    F64,
}

impl SampleFormat {
    pub fn bytes_per_sample(&self) -> usize {
        match self {
            SampleFormat::U8 => 1,
            SampleFormat::S16 => 2,
            SampleFormat::S24 => 3,
            SampleFormat::S32 => 4,
            SampleFormat::F32 => 4,
            SampleFormat::F64 => 8,
        }
    }

    pub fn is_float(&self) -> bool {
        matches!(self, SampleFormat::F32 | SampleFormat::F64)
    }

    pub fn is_integer(&self) -> bool {
        !self.is_float()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: SampleFormat,
}

impl AudioFormat {
    pub fn new(sample_rate: u32, channels: u16, sample_format: SampleFormat) -> Self {
        Self {
            sample_rate,
            channels,
            sample_format,
        }
    }

    pub fn bytes_per_frame(&self) -> usize {
        self.channels as usize * self.sample_format.bytes_per_sample()
    }

    pub fn frames_per_second(&self) -> u32 {
        self.sample_rate
    }

    pub fn bytes_per_second(&self) -> usize {
        self.bytes_per_frame() * self.sample_rate as usize
    }
}

impl std::fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} Hz, {} ch, {:?}",
            self.sample_rate, self.channels, self.sample_format
        )
    }
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self::new(44100, 2, SampleFormat::S16)
    }
}
