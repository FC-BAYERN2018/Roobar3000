use crate::utils::error::{AudioError, Result};
use crate::dsp::processor::DSPProcessor;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use tracing::{debug, info};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EQBand {
    pub frequency: f32,
    pub gain_db: f32,
    pub q: f32,
}

impl EQBand {
    pub fn new(frequency: f32, gain_db: f32, q: f32) -> Self {
        Self {
            frequency,
            gain_db,
            q,
        }
    }

    pub fn set_gain(&mut self, gain_db: f32) {
        self.gain_db = gain_db.clamp(-20.0, 20.0);
    }

    pub fn reset(&mut self) {
        self.gain_db = 0.0;
    }
}

impl Default for EQBand {
    fn default() -> Self {
        Self::new(1000.0, 0.0, 1.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EQPreset {
    pub name: String,
    pub bands: Vec<EQBand>,
}

impl EQPreset {
    pub fn new(name: &str, bands: Vec<EQBand>) -> Self {
        Self {
            name: name.to_string(),
            bands,
        }
    }

    pub fn flat() -> Self {
        let frequencies = vec![
            32.0, 64.0, 125.0, 250.0, 500.0,
            1000.0, 2000.0, 4000.0, 8000.0, 16000.0
        ];
        
        let bands = frequencies.iter().map(|&f| EQBand::new(f, 0.0, 1.414)).collect();
        
        Self::new("Flat", bands)
    }

    pub fn bass_boost() -> Self {
        let mut preset = Self::flat();
        preset.bands[0].gain_db = 6.0;
        preset.bands[1].gain_db = 4.0;
        preset.bands[2].gain_db = 2.0;
        preset.name = "Bass Boost".to_string();
        preset
    }

    pub fn vocal() -> Self {
        let mut preset = Self::flat();
        preset.bands[3].gain_db = -2.0;
        preset.bands[4].gain_db = 2.0;
        preset.bands[5].gain_db = 3.0;
        preset.bands[6].gain_db = 2.0;
        preset.name = "Vocal".to_string();
        preset
    }

    pub fn rock() -> Self {
        let mut preset = Self::flat();
        preset.bands[0].gain_db = 5.0;
        preset.bands[1].gain_db = 3.0;
        preset.bands[6].gain_db = 2.0;
        preset.bands[7].gain_db = 4.0;
        preset.bands[8].gain_db = 3.0;
        preset.name = "Rock".to_string();
        preset
    }

    pub fn classical() -> Self {
        let mut preset = Self::flat();
        preset.bands[0].gain_db = 4.0;
        preset.bands[1].gain_db = 3.0;
        preset.bands[2].gain_db = 2.0;
        preset.bands[7].gain_db = 2.0;
        preset.bands[8].gain_db = 3.0;
        preset.bands[9].gain_db = 2.0;
        preset.name = "Classical".to_string();
        preset
    }
}

pub struct Equalizer {
    bands: Vec<EQBand>,
    sample_rate: u32,
    channels: u16,
    enabled: bool,
    presets: HashMap<String, EQPreset>,
    current_preset: Option<String>,
    history: Vec<Vec<f32>>,
}

impl Equalizer {
    pub fn new(sample_rate: u32, channels: u16, band_count: usize) -> Self {
        let frequencies = Self::default_frequencies(band_count);
        let bands = frequencies.iter().map(|&f| EQBand::new(f, 0.0, 1.414)).collect();

        let mut presets = HashMap::new();
        presets.insert("Flat".to_string(), EQPreset::flat());
        presets.insert("Bass Boost".to_string(), EQPreset::bass_boost());
        presets.insert("Vocal".to_string(), EQPreset::vocal());
        presets.insert("Rock".to_string(), EQPreset::rock());
        presets.insert("Classical".to_string(), EQPreset::classical());

        info!("Equalizer created: {} bands, {} channels, {} Hz", 
            band_count, channels, sample_rate);

        Self {
            bands,
            sample_rate,
            channels,
            enabled: true,
            presets,
            current_preset: Some("Flat".to_string()),
            history: vec![vec![0.0; 2]; channels as usize],
        }
    }

    fn default_frequencies(count: usize) -> Vec<f32> {
        match count {
            10 => vec![
                32.0, 64.0, 125.0, 250.0, 500.0,
                1000.0, 2000.0, 4000.0, 8000.0, 16000.0
            ],
            8 => vec![
                60.0, 150.0, 400.0, 1000.0, 2400.0, 6000.0, 12000.0, 14000.0
            ],
            5 => vec![
                100.0, 400.0, 1000.0, 4000.0, 10000.0
            ],
            _ => {
                let mut freqs = Vec::with_capacity(count);
                let start = 20.0_f32.log10();
                let end = 20000.0_f32.log10();
                let step = (end - start) / (count - 1) as f32;
                
                for i in 0..count {
                    let log_freq = start + step * i as f32;
                    freqs.push(10.0_f32.powf(log_freq));
                }
                freqs
            }
        }
    }

    pub fn set_band_gain(&mut self, band_index: usize, gain_db: f32) -> Result<()> {
        if band_index >= self.bands.len() {
            return Err(AudioError::DSPError(format!("Invalid band index: {}", band_index)));
        }
        self.bands[band_index].set_gain(gain_db);
        self.current_preset = None;
        debug!("EQ band {} gain set to {:.1} dB", band_index, gain_db);
        Ok(())
    }

    pub fn get_band_gain(&self, band_index: usize) -> Option<f32> {
        self.bands.get(band_index).map(|band| band.gain_db)
    }

    pub fn get_bands(&self) -> &[EQBand] {
        &self.bands
    }

    pub fn set_bands(&mut self, bands: Vec<EQBand>) -> Result<()> {
        if bands.len() != self.bands.len() {
            return Err(AudioError::DSPError(format!(
                "Band count mismatch: expected {}, got {}", 
                self.bands.len(), bands.len()
            )));
        }
        self.bands = bands;
        self.current_preset = None;
        Ok(())
    }

    pub fn apply_preset(&mut self, preset_name: &str) -> Result<()> {
        if let Some(preset) = self.presets.get(preset_name) {
            self.bands = preset.bands.clone();
            self.current_preset = Some(preset_name.to_string());
            info!("Applied EQ preset: {}", preset_name);
            Ok(())
        } else {
            Err(AudioError::DSPError(format!("Preset not found: {}", preset_name)))
        }
    }

    pub fn save_preset(&mut self, name: &str) -> Result<()> {
        let preset = EQPreset::new(name, self.bands.clone());
        self.presets.insert(name.to_string(), preset);
        self.current_preset = Some(name.to_string());
        info!("Saved EQ preset: {}", name);
        Ok(())
    }

    pub fn get_presets(&self) -> Vec<String> {
        self.presets.keys().cloned().collect()
    }

    pub fn current_preset(&self) -> Option<&str> {
        self.current_preset.as_deref()
    }

    pub fn reset(&mut self) {
        for band in &mut self.bands {
            band.reset();
        }
        self.current_preset = Some("Flat".to_string());
        self.history = vec![vec![0.0; 2]; self.channels as usize];
        debug!("Equalizer reset");
    }

    fn calculate_biquad_coefficients(&self, band: &EQBand) -> (f32, f32, f32, f32, f32) {
        let a = 10.0_f32.powf(band.gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * band.frequency / self.sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * band.q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0)
    }

    fn process_sample(&mut self, sample: f32, channel: usize) -> f32 {
        let mut processed = sample;

        for band in &self.bands {
            if band.gain_db.abs() > 0.01 {
                let (b0, b1, b2, a1, a2) = self.calculate_biquad_coefficients(band);
                
                let history = &mut self.history[channel];
                let output = b0 * processed + b1 * history[0] + b2 * history[1] 
                           - a1 * history[0] - a2 * history[1];
                
                history[1] = history[0];
                history[0] = processed;
                processed = output;
            }
        }

        processed
    }
}

impl DSPProcessor for Equalizer {
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        if input.len() != output.len() {
            return Err(AudioError::DSPError("Input and output buffer sizes must match".into()));
        }

        if !self.enabled {
            output.copy_from_slice(input);
            return Ok(());
        }

        let samples_per_channel = input.len() / self.channels as usize;

        for ch in 0..self.channels as usize {
            for i in 0..samples_per_channel {
                let idx = i * self.channels as usize + ch;
                output[idx] = self.process_sample(input[idx], ch);
            }
        }

        Ok(())
    }

    fn reset(&mut self) {
        self.reset();
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        debug!("Equalizer enabled: {}", enabled);
    }

    fn get_name(&self) -> &str {
        "Equalizer"
    }
}

impl Default for Equalizer {
    fn default() -> Self {
        Self::new(44100, 2, 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eq_band_creation() {
        let band = EQBand::new(1000.0, 3.0, 1.414);
        assert_eq!(band.frequency, 1000.0);
        assert_eq!(band.gain_db, 3.0);
    }

    #[test]
    fn test_eq_gain_clamping() {
        let mut band = EQBand::new(1000.0, 0.0, 1.0);
        band.set_gain(30.0);
        assert_eq!(band.gain_db, 20.0);
        
        band.set_gain(-30.0);
        assert_eq!(band.gain_db, -20.0);
    }

    #[test]
    fn test_preset_application() {
        let mut eq = Equalizer::new(44100, 2, 10);
        eq.apply_preset("Bass Boost").unwrap();
        
        assert!(eq.get_band_gain(0).unwrap() > 0.0);
        assert!(eq.get_band_gain(1).unwrap() > 0.0);
    }

    #[test]
    fn test_preset_save() {
        let mut eq = Equalizer::new(44100, 2, 10);
        eq.set_band_gain(0, 5.0).unwrap();
        eq.save_preset("Custom").unwrap();
        
        let presets = eq.get_presets();
        assert!(presets.contains(&"Custom".to_string()));
    }
}
