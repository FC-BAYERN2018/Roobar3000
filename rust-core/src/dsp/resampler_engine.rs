use crate::audio::format::AudioFormat;
use crate::utils::error::{AudioError, Result};
use crate::dsp::processor::DSPProcessor;
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use tracing::{info, debug};

pub struct ResamplerEngine {
    resampler: Option<SincFixedIn<f32>>,
    input_rate: u32,
    output_rate: u32,
    channels: u16,
    chunk_size: usize,
    enabled: bool,
    quality: ResampleQuality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResampleQuality {
    Low,
    Medium,
    High,
    VeryHigh,
}

impl ResampleQuality {
    fn sinc_len(&self) -> usize {
        match self {
            ResampleQuality::Low => 64,
            ResampleQuality::Medium => 128,
            ResampleQuality::High => 256,
            ResampleQuality::VeryHigh => 512,
        }
    }

    fn oversampling_factor(&self) -> usize {
        match self {
            ResampleQuality::Low => 32,
            ResampleQuality::Medium => 64,
            ResampleQuality::High => 128,
            ResampleQuality::VeryHigh => 256,
        }
    }
}

impl ResamplerEngine {
    pub fn new(input_format: AudioFormat, output_rate: u32, quality: ResampleQuality) -> Result<Self> {
        if input_format.sample_rate == output_rate {
            return Ok(Self {
                resampler: None,
                input_rate: input_format.sample_rate,
                output_rate,
                channels: input_format.channels,
                chunk_size: 1024,
                enabled: true,
                quality,
            });
        }

        let sinc_params = SincInterpolationParameters {
            sinc_len: quality.sinc_len(),
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Cubic,
            oversampling_factor: quality.oversampling_factor(),
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

        info!("ResamplerEngine created: {} Hz -> {} Hz, quality: {:?}", 
            input_format.sample_rate, output_rate, quality);

        Ok(Self {
            resampler: Some(resampler),
            input_rate: input_format.sample_rate,
            output_rate,
            channels: input_format.channels,
            chunk_size,
            enabled: true,
            quality,
        })
    }

    pub fn set_quality(&mut self, quality: ResampleQuality) -> Result<()> {
        if self.quality != quality {
            self.quality = quality;
            if self.needs_resampling() {
                let sinc_params = SincInterpolationParameters {
                    sinc_len: quality.sinc_len(),
                    f_cutoff: 0.95,
                    interpolation: SincInterpolationType::Cubic,
                    oversampling_factor: quality.oversampling_factor(),
                    window: WindowFunction::BlackmanHarris2,
                };

                self.resampler = Some(SincFixedIn::new(
                    self.output_rate as f64 / self.input_rate as f64,
                    2.0,
                    sinc_params,
                    self.chunk_size,
                    self.channels as usize,
                ).map_err(|e| AudioError::ResampleError(format!("Failed to update resampler: {}", e)))?);

                debug!("Resampler quality updated to {:?}", quality);
            }
        }
        Ok(())
    }

    pub fn get_quality(&self) -> ResampleQuality {
        self.quality
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize> {
        if let Some(resampler) = &mut self.resampler {
            let frames_in = input.len() / self.channels as usize;
            let input_buffer = vec![input.to_vec()];
            
            let resampled = resampler.process(&input_buffer, None)
                .map_err(|e| AudioError::ResampleError(format!("Resampling failed: {}", e)))?;
            
            let output_flat: Vec<f32> = resampled.into_iter().flatten().collect();
            let frames_out = output_flat.len().min(output.len());
            
            output[..frames_out].copy_from_slice(&output_flat[..frames_out]);
            
            debug!("Resampled {} frames to {} frames", frames_in, frames_out / self.channels as usize);
            
            Ok(frames_out)
        } else {
            let frames = input.len().min(output.len());
            output[..frames].copy_from_slice(&input[..frames]);
            Ok(frames)
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

impl DSPProcessor for ResamplerEngine {
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        self.process(input, output)?;
        Ok(())
    }

    fn reset(&mut self) {
        if let Some(resampler) = &mut self.resampler {
            resampler.reset();
        }
        debug!("ResamplerEngine reset");
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn get_name(&self) -> &str {
        "Resampler"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::SampleFormat;

    #[test]
    fn test_no_resampling_needed() {
        let format = AudioFormat::new(44100, 2, SampleFormat::F32);
        let resampler = ResamplerEngine::new(format, 44100, ResampleQuality::High).unwrap();
        assert!(!resampler.needs_resampling());
    }

    #[test]
    fn test_resampling_needed() {
        let format = AudioFormat::new(44100, 2, SampleFormat::F32);
        let resampler = ResamplerEngine::new(format, 48000, ResampleQuality::High).unwrap();
        assert!(resampler.needs_resampling());
        assert_eq!(resampler.input_rate(), 44100);
        assert_eq!(resampler.output_rate(), 48000);
    }

    #[test]
    fn test_quality_settings() {
        let format = AudioFormat::new(44100, 2, SampleFormat::F32);
        let mut resampler = ResamplerEngine::new(format, 48000, ResampleQuality::Medium).unwrap();
        assert_eq!(resampler.get_quality(), ResampleQuality::Medium);
        
        resampler.set_quality(ResampleQuality::VeryHigh).unwrap();
        assert_eq!(resampler.get_quality(), ResampleQuality::VeryHigh);
    }
}
