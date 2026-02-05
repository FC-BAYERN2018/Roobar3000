use crate::audio::format::AudioFormat;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct ClockStats {
    pub drift_ppm: f64,
    pub jitter_ns: u64,
    pub buffer_level: f32,
}

pub struct AudioClock {
    format: AudioFormat,
    start_time: Option<Instant>,
    frames_played: u64,
    last_update: Option<Instant>,
    drift_accumulator: f64,
    jitter_samples: Vec<u64>,
}

impl AudioClock {
    pub fn new(format: AudioFormat) -> Self {
        Self {
            format,
            start_time: None,
            frames_played: 0,
            last_update: None,
            drift_accumulator: 0.0,
            jitter_samples: Vec::with_capacity(100),
        }
    }

    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        self.last_update = Some(Instant::now());
        self.frames_played = 0;
        self.drift_accumulator = 0.0;
        self.jitter_samples.clear();
    }

    pub fn stop(&mut self) {
        self.start_time = None;
        self.last_update = None;
    }

    pub fn is_running(&self) -> bool {
        self.start_time.is_some()
    }

    pub fn update(&mut self, frames: u64) {
        if !self.is_running() {
            return;
        }

        let now = Instant::now();
        if let Some(last) = self.last_update {
            let elapsed = now.duration_since(last);
            let expected_frames = (elapsed.as_secs_f64() * self.format.sample_rate as f64) as u64;
            
            if frames > expected_frames {
                let drift = (frames - expected_frames) as f64;
                self.drift_accumulator += drift;
            } else if expected_frames > frames {
                let drift = (expected_frames - frames) as f64;
                self.drift_accumulator -= drift;
            }

            self.jitter_samples.push(frames.abs_diff(expected_frames));
            if self.jitter_samples.len() > 100 {
                self.jitter_samples.remove(0);
            }
        }

        self.frames_played += frames;
        self.last_update = Some(now);
    }

    pub fn get_position(&self) -> Option<Duration> {
        self.start_time.map(|_start| {
            let frames_duration = self.frames_played as f64 / self.format.sample_rate as f64;
            Duration::from_secs_f64(frames_duration)
        })
    }

    pub fn get_stats(&self) -> Option<ClockStats> {
        if !self.is_running() {
            return None;
        }

        let total_frames = self.frames_played as f64;
        let elapsed = self.start_time.unwrap().elapsed().as_secs_f64();
        let expected_frames = elapsed * self.format.sample_rate as f64;
        
        let drift_ppm = if expected_frames > 0.0 {
            (total_frames - expected_frames) / expected_frames * 1_000_000.0
        } else {
            0.0
        };

        let jitter_ns = if !self.jitter_samples.is_empty() {
            let sum: u64 = self.jitter_samples.iter().sum();
            let avg = sum / self.jitter_samples.len() as u64;
            let variance: f64 = self.jitter_samples
                .iter()
                .map(|&x| {
                    let diff = x as f64 - avg as f64;
                    diff * diff
                })
                .sum::<f64>() / self.jitter_samples.len() as f64;
            (variance.sqrt() * 1_000_000_000.0 / self.format.sample_rate as f64) as u64
        } else {
            0
        };

        Some(ClockStats {
            drift_ppm,
            jitter_ns,
            buffer_level: 0.0,
        })
    }

    pub fn reset(&mut self) {
        self.frames_played = 0;
        self.drift_accumulator = 0.0;
        self.jitter_samples.clear();
        if self.is_running() {
            self.start_time = Some(Instant::now());
            self.last_update = Some(Instant::now());
        }
    }
}

#[derive(Clone)]
pub struct ClockSync {
    #[allow(dead_code)]
    target_drift_ppm: f64,
    correction_threshold: f64,
}

impl ClockSync {
    pub fn new() -> Self {
        Self {
            target_drift_ppm: 10.0,
            correction_threshold: 50.0,
        }
    }

    pub fn with_threshold(mut self, ppm: f64) -> Self {
        self.correction_threshold = ppm;
        self
    }

    pub fn needs_correction(&self, stats: &ClockStats) -> bool {
        stats.drift_ppm.abs() > self.correction_threshold
    }

    pub fn calculate_correction(&self, stats: &ClockStats) -> f64 {
        if !self.needs_correction(stats) {
            0.0
        } else {
            -stats.drift_ppm / 1_000_000.0
        }
    }
}

impl Default for ClockSync {
    fn default() -> Self {
        Self::new()
    }
}
