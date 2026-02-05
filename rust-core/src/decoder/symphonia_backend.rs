use crate::audio::format::{AudioFormat, SampleFormat};
use crate::audio::buffer_pool::AudioBuffer;
use crate::utils::error::{AudioError, Result};
use symphonia::core::codecs::{Decoder as SymphoniaDecoder, CODEC_TYPE_NULL};
use symphonia::core::formats::{FormatReader, FormatOptions};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::audio::{SampleBuffer, SignalSpec};
use symphonia::default::{get_codecs, get_probe};
use std::fs::File;
use std::path::Path;
use tracing::{info, debug, warn};

pub struct Decoder {
    format_reader: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaDecoder>,
    format: AudioFormat,
    total_frames: Option<u64>,
    current_frame: u64,
    sample_buffer: Option<SampleBuffer<f32>>,
}

impl Decoder {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        
        let file = File::open(path).map_err(|e| {
            AudioError::IoError(format!("Failed to open file {}: {}", path.display(), e))
        })?;

        let media_source: Box<dyn MediaSource> = Box::new(file);
        let mss = MediaSourceStream::new(media_source, Default::default());

        let hint = Hint::new();
        let meta_opts: MetadataOptions = Default::default();
        let mut fmt_opts = FormatOptions::default();
        fmt_opts.enable_gapless = true;

        let probed = get_probe().format(&hint, mss, &fmt_opts, &meta_opts).map_err(|e| {
            AudioError::DecodeError(format!("Failed to probe format: {}", e))
        })?;

        let format_reader = probed.format;
        let track = format_reader.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| AudioError::DecodeError("No valid audio track found".into()))?;

        let codec_params = &track.codec_params;

        let sample_rate = codec_params.sample_rate.ok_or_else(|| {
            AudioError::DecodeError("Sample rate not found".into())
        })?;

        let channels = codec_params.channels.ok_or_else(|| {
            AudioError::DecodeError("Channels not found".into())
        })?.count();

        let bits_per_sample = codec_params.bits_per_sample.unwrap_or(16);

        let sample_format = match bits_per_sample {
            8 => SampleFormat::U8,
            16 => SampleFormat::S16,
            24 => SampleFormat::S24,
            32 => {
                if codec_params.codec == CODEC_TYPE_NULL {
                    SampleFormat::F32
                } else {
                    SampleFormat::S32
                }
            }
            _ => SampleFormat::S16,
        };

        let audio_format = AudioFormat::new(sample_rate, channels as u16, sample_format);

        let decoder = get_codecs().make(&track.codec_params, &Default::default())
            .map_err(|e| AudioError::DecodeError(format!("Failed to create decoder: {}", e)))?;

        let total_frames = codec_params.n_frames;

        info!("Symphonia decoder created: {}Hz, {}ch, {}bit", 
            sample_rate, channels, bits_per_sample);

        Ok(Self {
            format_reader,
            decoder,
            format: audio_format,
            total_frames,
            current_frame: 0,
            sample_buffer: None,
        })
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
            match self.format_reader.next_packet() {
                Ok(packet) => {
                    match self.decoder.decode(&packet) {
                        Ok(decoded) => {
                            // 如果sample_buffer不存在或大小不匹配，重新创建
                            if self.sample_buffer.is_none() || 
                               self.sample_buffer.as_ref().unwrap().capacity() < decoded.frames() {
                                let spec = SignalSpec::new(decoded.spec().rate, decoded.spec().channels);
                                self.sample_buffer = Some(SampleBuffer::new(decoded.frames() as u64, spec));
                            }

                            // 将解码的数据复制到sample_buffer
                            if let Some(sample_buf) = &mut self.sample_buffer {
                                sample_buf.copy_interleaved_ref(decoded);
                                
                                let frames_to_copy = (sample_buf.len() / self.format.channels as usize)
                                    .min(target_frames - decoded_frames);
                                let samples_to_copy = frames_to_copy * self.format.channels as usize;

                                if frames_to_copy == 0 {
                                    break;
                                }

                                let offset = decoded_frames * bytes_per_frame;
                                let buffer_data = buffer.data_mut();

                                // 复制样本到输出缓冲区
                                for i in 0..samples_to_copy {
                                    let sample = sample_buf.samples()[i];
                                    let byte_offset = offset + i * (bytes_per_frame / self.format.channels as usize);
                                    if byte_offset + 4 <= buffer_data.len() {
                                        let bytes: [u8; 4] = sample.to_le_bytes();
                                        buffer_data[byte_offset..byte_offset + 4].copy_from_slice(&bytes);
                                    }
                                }

                                decoded_frames += frames_to_copy;
                                self.current_frame += frames_to_copy as u64;

                                if frames_to_copy < sample_buf.len() / self.format.channels as usize {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            let err_str = e.to_string();
                            if err_str.contains("end of stream") || err_str.contains("EOF") {
                                break;
                            }
                            warn!("Decode error: {}", err_str);
                            break;
                        }
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("end of stream") || err_str.contains("EOF") {
                        break;
                    }
                    warn!("Packet error: {}", err_str);
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
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        debug!("Symphonia decoder dropped");
    }
}
