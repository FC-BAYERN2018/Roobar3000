use std::sync::Arc;
use parking_lot::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct AudioMetrics {
    pub buffer_underruns: u64,
    pub buffer_overruns: u64,
    pub decode_errors: u64,
    pub output_errors: u64,
    pub total_frames_decoded: u64,
    pub total_frames_output: u64,
    pub average_latency_ms: f64,
    pub peak_latency_ms: f64,
    pub jitter_ns: u64,
    pub clock_drift_ppm: f64,
}

impl Default for AudioMetrics {
    fn default() -> Self {
        Self {
            buffer_underruns: 0,
            buffer_overruns: 0,
            decode_errors: 0,
            output_errors: 0,
            total_frames_decoded: 0,
            total_frames_output: 0,
            average_latency_ms: 0.0,
            peak_latency_ms: 0.0,
            jitter_ns: 0,
            clock_drift_ppm: 0.0,
        }
    }
}

pub struct MetricsCollector {
    metrics: Arc<Mutex<AudioMetrics>>,
    latency_samples: Arc<Mutex<Vec<f64>>>,
    start_time: Instant,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(AudioMetrics::default())),
            latency_samples: Arc::new(Mutex::new(Vec::with_capacity(1000))),
            start_time: Instant::now(),
        }
    }

    pub fn record_buffer_underrun(&self) {
        let mut metrics = self.metrics.lock();
        metrics.buffer_underruns += 1;
    }

    pub fn record_buffer_overrun(&self) {
        let mut metrics = self.metrics.lock();
        metrics.buffer_overruns += 1;
    }

    pub fn record_decode_error(&self) {
        let mut metrics = self.metrics.lock();
        metrics.decode_errors += 1;
    }

    pub fn record_output_error(&self) {
        let mut metrics = self.metrics.lock();
        metrics.output_errors += 1;
    }

    pub fn record_frames_decoded(&self, frames: u64) {
        let mut metrics = self.metrics.lock();
        metrics.total_frames_decoded += frames;
    }

    pub fn record_frames_output(&self, frames: u64) {
        let mut metrics = self.metrics.lock();
        metrics.total_frames_output += frames;
    }

    pub fn record_latency(&self, latency_ms: f64) {
        let mut samples = self.latency_samples.lock();
        samples.push(latency_ms);
        
        if samples.len() > 1000 {
            samples.remove(0);
        }

        let mut metrics = self.metrics.lock();
        metrics.peak_latency_ms = metrics.peak_latency_ms.max(latency_ms);
        
        let sum: f64 = samples.iter().sum();
        metrics.average_latency_ms = sum / samples.len() as f64;
    }

    pub fn record_jitter(&self, jitter_ns: u64) {
        let mut metrics = self.metrics.lock();
        metrics.jitter_ns = jitter_ns;
    }

    pub fn record_clock_drift(&self, drift_ppm: f64) {
        let mut metrics = self.metrics.lock();
        metrics.clock_drift_ppm = drift_ppm;
    }

    pub fn get_metrics(&self) -> AudioMetrics {
        self.metrics.lock().clone()
    }

    pub fn reset(&self) {
        let mut metrics = self.metrics.lock();
        *metrics = AudioMetrics::default();
        
        let mut samples = self.latency_samples.lock();
        samples.clear();
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn get_performance_report(&self) -> PerformanceReport {
        let metrics = self.get_metrics();
        let uptime = self.uptime();
        
        let decode_error_rate = if metrics.total_frames_decoded > 0 {
            (metrics.decode_errors as f64 / metrics.total_frames_decoded as f64) * 100.0
        } else {
            0.0
        };

        let output_error_rate = if metrics.total_frames_output > 0 {
            (metrics.output_errors as f64 / metrics.total_frames_output as f64) * 100.0
        } else {
            0.0
        };

        let frame_loss_rate = if metrics.total_frames_decoded > 0 {
            let lost = metrics.total_frames_decoded.saturating_sub(metrics.total_frames_output);
            (lost as f64 / metrics.total_frames_decoded as f64) * 100.0
        } else {
            0.0
        };

        PerformanceReport {
            metrics: metrics.clone(),
            uptime,
            decode_error_rate,
            output_error_rate,
            frame_loss_rate,
            health_score: Self::calculate_health_score(&metrics),
        }
    }

    fn calculate_health_score(metrics: &AudioMetrics) -> f64 {
        let mut score = 100.0;

        score -= (metrics.buffer_underruns as f64 * 0.5).min(20.0);
        score -= (metrics.buffer_overruns as f64 * 0.2).min(10.0);
        score -= (metrics.decode_errors as f64 * 1.0).min(30.0);
        score -= (metrics.output_errors as f64 * 1.0).min(30.0);
        
        if metrics.average_latency_ms > 100.0 {
            score -= (metrics.average_latency_ms - 100.0) * 0.1;
        }

        if metrics.jitter_ns > 1_000_000 {
            score -= (metrics.jitter_ns as f64 / 1_000_000.0 - 1.0) * 5.0;
        }

        if metrics.clock_drift_ppm.abs() > 100.0 {
            score -= (metrics.clock_drift_ppm.abs() - 100.0) * 0.1;
        }

        score.max(0.0).min(100.0)
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub metrics: AudioMetrics,
    pub uptime: Duration,
    pub decode_error_rate: f64,
    pub output_error_rate: f64,
    pub frame_loss_rate: f64,
    pub health_score: f64,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
