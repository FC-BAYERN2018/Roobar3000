use crate::audio::format::{AudioFormat, SampleFormat};
use crate::utils::error::{AudioError, Result};
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use tracing::{info, debug};

pub struct Resampler {
    resampler: Option<SincFixedIn<f32>>,
    input_rate: u32,
    output_rate: u32,
    channels: u16,
    chunk_size: usize,
}

impl Resampler {
    pub fn new(input_format: AudioFormat, output_rate: u32) -> Result<Self> {
        if input_format.sample_rate == output_rate {
            return Ok(Self {
                resampler: None,
                input_rate: input_format.sample_rate,
                output_rate,
                channels: input_format.channels,
                chunk_size: 1024,
            });
        }

        let sinc_params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Cubic,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };

        let chunk_size = 1024;
        let resampler = SincFixedIn::new(
            output_rate as f64 / input_format.sample_rate as f64,
            2.0,
            sinc_params,
            chunk_size,
            input_format.channels as usize,
        ).map_err(|e| AudioError::ResampleError(format!("Failed to create resampler: {}", e)))?;

        info!("Resampler created: {} Hz -> {} Hz, {} channels", 
            input_format.sample_rate, output_rate, input_format.channels);

        Ok(Self {
            resampler: Some(resampler),
            input_rate: input_format.sample_rate,
            output_rate,
            channels: input_format.channels,
            chunk_size,
        })
    }

    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        if let Some(resampler) = &mut self.resampler {
            let frames_in = input.len() / self.channels as usize;
            let input_buffer = vec![input.to_vec()];
            
            let output = resampler.process(&input_buffer, None)
                .map_err(|e| AudioError::ResampleError(format!("Resampling failed: {}", e)))?;
            
            let output_flat: Vec<f32> = output.into_iter().flatten().collect();
            debug!("Resampled {} frames to {} frames", frames_in, output_flat.len() / self.channels as usize);
            
            Ok(output_flat)
        } else {
            Ok(input.to_vec())
        }
    }

    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }

    pub fn output_rate(&self) -> u32 {
        self.output_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn ratio(&self) -> f64 {
        self.output_rate as f64 / self.input_rate as f64
    }

    pub fn needs_resampling(&self) -> bool {
        self.input_rate != self.output_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_resampling_needed() {
        let format = AudioFormat::new(44100, 2, SampleFormat::F32);
        let resampler = Resampler::new(format, 44100).unwrap();
        assert!(!resampler.needs_resampling());
    }

    #[test]
    fn test_resampling_needed() {
        let format = AudioFormat::new(44100, 2, SampleFormat::F32);
        let resampler = Resampler::new(format, 48000).unwrap();
        assert!(resampler.needs_resampling());
        assert_eq!(resampler.input_rate(), 44100);
        assert_eq!(resampler.output_rate(), 48000);
    }
}
