use crate::audio::format::{AudioFormat, SampleFormat};
use crate::audio::buffer_pool::AudioBuffer;
use crate::utils::error::{AudioError, Result};
use symphonia::core::codecs::{Decoder as SymphoniaDecoder, CODEC_TYPE_NULL};
use symphonia::core::conv::{FromSample, IntoSample};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::{get_codecs, get_probe};
use std::fs::File;
use std::path::Path;
use tracing::{info, debug, warn};

pub struct Decoder {
    decoder: Box<dyn SymphoniaDecoder>,
    format: AudioFormat,
    total_frames: Option<u64>,
    current_frame: u64,
    sample_buffer: Vec<f32>,
}

impl Decoder {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        
        let file = Box::new(File::open(path).map_err(|e| {
            AudioError::IoError(format!("Failed to open file {}: {}", path.display(), e))
        })?);

        let mss = MediaSourceStream::new(file, Default::default());

        let hint = Hint::new();
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        let probed = get_probe().format(&hint, mss, &fmt_opts, &meta_opts).map_err(|e| {
            AudioError::DecodeError(format!("Failed to probe format: {}", e))
        })?;

        let mut format = probed.format;

        let track = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| AudioError::DecodeError("No valid audio track found".into()))?;

        let codec_params = &track.codec_params;

        let sample_rate = codec_params.sample_rate.ok_or_else(|| {
            AudioError::DecodeError("Sample rate not found".into())
        })?;

        let channels = codec_params.channels.ok_or_else(|| {
            AudioError::DecodeError("Channels not found".into())
        })?.count() as u16;

        let sample_format = Self::determine_sample_format(codec_params)?;

        let audio_format = AudioFormat::new(sample_rate, channels, sample_format);

        let mut decoder = get_codecs().make(&codec_params, &Default::default())
            .map_err(|e| AudioError::DecodeError(format!("Failed to create decoder: {}", e)))?;

        let total_frames = codec_params.n_frames;

        info!("Decoder created: format={}, sample_rate={}, channels={}", 
            audio_format, sample_rate, channels);

        Ok(Self {
            decoder,
            format: audio_format,
            total_frames,
            current_frame: 0,
            sample_buffer: Vec::new(),
        })
    }

    fn determine_sample_format(codec_params: &symphonia::core::codec::CodecParameters) -> Result<SampleFormat> {
        if let Some(bits_per_sample) = codec_params.bits_per_sample {
            match bits_per_sample {
                8 => Ok(SampleFormat::U8),
                16 => Ok(SampleFormat::S16),
                24 => Ok(SampleFormat::S24),
                32 => {
                    if codec_params.codec == symphonia::core::codecs::CODEC_TYPE_PCM_F32LE {
                        Ok(SampleFormat::F32)
                    } else {
                        Ok(SampleFormat::S32)
                    }
                }
                64 => Ok(SampleFormat::F64),
                _ => Ok(SampleFormat::S16),
            }
        } else {
            Ok(SampleFormat::S16)
        }
    }

    pub fn format(&self) -> AudioFormat {
        self.format
    }

    pub fn total_frames(&self) -> Option<u64> {
        self.total_frames
    }

    pub fn current_frame(&self) -> u64 {
        self.current_frame
    }

    pub fn duration(&self) -> Option<std::time::Duration> {
        self.total_frames.map(|frames| {
            let secs = frames as f64 / self.format.sample_rate as f64;
            std::time::Duration::from_secs_f64(secs)
        })
    }

    pub fn decode_next(&mut self, buffer: &mut AudioBuffer) -> Result<usize> {
        let target_frames = buffer.frames();
        let bytes_per_frame = self.format.bytes_per_frame();
        let mut decoded_frames = 0;

        while decoded_frames < target_frames {
            match self.decoder.decode(&mut self.sample_buffer) {
                Ok(decoded) => {
                    if decoded.is_empty() {
                        break;
                    }

                    let frames_to_copy = (decoded.len() / self.format.channels as usize).min(target_frames - decoded_frames);
                    let samples_to_copy = frames_to_copy * self.format.channels as usize;

                    let offset = decoded_frames * bytes_per_frame;
                    let data = buffer.data_mut();

                    for (i, &sample) in decoded.iter().take(samples_to_copy).enumerate() {
                        let byte_offset = offset + i * (bytes_per_frame / self.format.channels as usize);
                        if byte_offset + 4 <= data.len() {
                            let bytes = sample.to_le_bytes();
                            data[byte_offset..byte_offset + 4].copy_from_slice(&bytes);
                        }
                    }

                    decoded_frames += frames_to_copy;
                    self.current_frame += frames_to_copy as u64;
                }
                Err(e) => {
                    if e.to_string().contains("end of stream") {
                        break;
                    }
                    warn!("Decode error: {}", e);
                    break;
                }
            }
        }

        debug!("Decoded {} frames", decoded_frames);
        Ok(decoded_frames)
    }

    pub fn seek(&mut self, frame: u64) -> Result<()> {
        self.current_frame = frame;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.sample_buffer.clear();
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        debug!("Decoder dropped");
    }
}
