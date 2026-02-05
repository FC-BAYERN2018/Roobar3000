use crate::utils::error::{AudioError, Result};
use tracing::{debug, trace};

pub trait DSPProcessor: Send + Sync {
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()>;
    fn reset(&mut self);
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn get_name(&self) -> &str;
}

pub struct DSPChain {
    processors: Vec<Box<dyn DSPProcessor>>,
    enabled: bool,
}

impl DSPChain {
    pub fn new() -> Self {
        Self {
            processors: Vec::new(),
            enabled: true,
        }
    }

    pub fn add_processor(&mut self, processor: Box<dyn DSPProcessor>) {
        debug!("Adding DSP processor: {}", processor.get_name());
        self.processors.push(processor);
    }

    pub fn remove_processor(&mut self, name: &str) -> Option<Box<dyn DSPProcessor>> {
        if let Some(pos) = self.processors.iter().position(|p| p.get_name() == name) {
            debug!("Removing DSP processor: {}", name);
            Some(self.processors.remove(pos))
        } else {
            None
        }
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        if !self.enabled {
            output.copy_from_slice(input);
            return Ok(());
        }

        let mut buffer = input.to_vec();
        
        for processor in &mut self.processors {
            if processor.is_enabled() {
                let mut temp_buffer = buffer.clone();
                processor.process(&buffer, &mut temp_buffer)?;
                buffer = temp_buffer;
            }
        }

        output.copy_from_slice(&buffer);
        trace!("DSP chain processed {} samples", input.len());
        Ok(())
    }

    pub fn reset(&mut self) {
        for processor in &mut self.processors {
            processor.reset();
        }
        debug!("DSP chain reset");
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        debug!("DSP chain enabled: {}", enabled);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn processor_count(&self) -> usize {
        self.processors.len()
    }

    pub fn get_processor_names(&self) -> Vec<String> {
        self.processors.iter().map(|p| p.get_name().to_string()).collect()
    }
}

impl Default for DSPChain {
    fn default() -> Self {
        Self::new()
    }
}

pub struct VolumeProcessor {
    volume: f32,
    enabled: bool,
}

impl VolumeProcessor {
    pub fn new(initial_volume: f32) -> Self {
        Self {
            volume: initial_volume.clamp(0.0, 2.0),
            enabled: true,
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 2.0);
    }

    pub fn get_volume(&self) -> f32 {
        self.volume
    }
}

impl DSPProcessor for VolumeProcessor {
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        if input.len() != output.len() {
            return Err(AudioError::DSPError("Input and output buffer sizes must match".into()));
        }

        for (i, &sample) in input.iter().enumerate() {
            output[i] = sample * self.volume;
        }

        Ok(())
    }

    fn reset(&mut self) {
        self.volume = 1.0;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn get_name(&self) -> &str {
        "Volume"
    }
}

pub struct GainProcessor {
    gain_db: f32,
    enabled: bool,
}

impl GainProcessor {
    pub fn new(gain_db: f32) -> Self {
        Self {
            gain_db,
            enabled: true,
        }
    }

    pub fn set_gain_db(&mut self, gain_db: f32) {
        self.gain_db = gain_db;
    }

    pub fn get_gain_db(&self) -> f32 {
        self.gain_db
    }

    fn db_to_linear(&self) -> f32 {
        10.0_f32.powf(self.gain_db / 20.0)
    }
}

impl DSPProcessor for GainProcessor {
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        if input.len() != output.len() {
            return Err(AudioError::DSPError("Input and output buffer sizes must match".into()));
        }

        let linear_gain = self.db_to_linear();
        for (i, &sample) in input.iter().enumerate() {
            output[i] = sample * linear_gain;
        }

        Ok(())
    }

    fn reset(&mut self) {
        self.gain_db = 0.0;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn get_name(&self) -> &str {
        "Gain"
    }
}

pub struct PassthroughProcessor;

impl DSPProcessor for PassthroughProcessor {
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        output.copy_from_slice(input);
        Ok(())
    }

    fn reset(&mut self) {}

    fn is_enabled(&self) -> bool {
        true
    }

    fn set_enabled(&mut self, _enabled: bool) {}

    fn get_name(&self) -> &str {
        "Passthrough"
    }
}
