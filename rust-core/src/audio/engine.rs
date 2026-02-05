use crate::audio::player::{Player, PlayerState, PlayerEvent};
use crate::audio::format::AudioFormat;
use crate::audio::buffer_pool::{BufferPool, SharedRingBuffer};
use crate::audio::clock::{AudioClock, ClockSync};
use crate::decoder::Decoder;
use crate::output::AudioOutput;
use crate::dsp::DSPProcessor;
use crate::utils::error::Result;
use crate::config::audio::AudioConfig;
use crossbeam_channel::{Sender, Receiver, unbounded};
use parking_lot::Mutex;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{info, error, debug, warn};

pub struct AudioEngine {
    player: Arc<Player>,
    decoder: Arc<Mutex<Option<Decoder>>>,
    #[allow(dead_code)]
    output: Arc<Mutex<Option<AudioOutput>>>,
    #[allow(dead_code)]
    dsp: Arc<Mutex<Option<Box<dyn DSPProcessor>>>>,
    buffer_pool: Arc<Mutex<BufferPool>>,
    ring_buffer: SharedRingBuffer,
    clock: Arc<Mutex<AudioClock>>,
    clock_sync: ClockSync,
    config: AudioConfig,
    command_sender: Sender<EngineCommand>,
    command_receiver: Receiver<EngineCommand>,
    event_sender: Sender<PlayerEvent>,
    worker_handle: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone)]
pub enum EngineCommand {
    LoadTrack(String),
    Play,
    Pause,
    Stop,
    Seek(Duration),
    SetVolume(f32),
    SetFormat(AudioFormat),
    EnableDSP(bool),
    SetEQ(Vec<f32>),
    Shutdown,
}

impl AudioEngine {
    pub fn new(config: AudioConfig) -> Result<Self> {
        let format = AudioFormat::default();
        let buffer_size_frames = config.buffer_size_frames;
        let pool_size = config.buffer_pool_size;
        let ring_buffer_size = config.ring_buffer_size;

        let buffer_pool = BufferPool::new(format, buffer_size_frames, pool_size);
        let ring_buffer = SharedRingBuffer::new(ring_buffer_size);
        let clock = AudioClock::new(format);
        let clock_sync = ClockSync::new();

        let (command_sender, command_receiver) = unbounded();
        let (event_sender, _event_receiver) = unbounded();

        let player = Player::new();

        info!("AudioEngine initialized with format: {}", format);

        Ok(Self {
            player: Arc::new(player),
            decoder: Arc::new(Mutex::new(None)),
            output: Arc::new(Mutex::new(None)),
            dsp: Arc::new(Mutex::new(None)),
            buffer_pool: Arc::new(Mutex::new(buffer_pool)),
            ring_buffer,
            clock: Arc::new(Mutex::new(clock)),
            clock_sync,
            config,
            command_sender,
            command_receiver,
            event_sender,
            worker_handle: None,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        if self.worker_handle.is_some() {
            return Ok(());
        }

        let player = Arc::clone(&self.player);
        let decoder = Arc::clone(&self.decoder);
        let buffer_pool = Arc::clone(&self.buffer_pool);
        let ring_buffer = self.ring_buffer.clone();
        let clock = Arc::clone(&self.clock);
        let clock_sync = self.clock_sync.clone();
        let config = self.config.clone();
        let receiver = self.command_receiver.clone();
        let event_sender = self.event_sender.clone();

        let handle = thread::spawn(move || {
            engine_worker(
                player,
                decoder,
                buffer_pool,
                ring_buffer,
                clock,
                clock_sync,
                config,
                receiver,
                event_sender,
            );
        });

        self.worker_handle = Some(handle);
        info!("AudioEngine worker started");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        let _ = self.command_sender.send(EngineCommand::Shutdown);
        
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }

        info!("AudioEngine stopped");
        Ok(())
    }

    pub fn command_sender(&self) -> &Sender<EngineCommand> {
        &self.command_sender
    }

    pub fn event_receiver(&self) -> &Receiver<PlayerEvent> {
        self.player.event_receiver()
    }

    pub fn player(&self) -> Arc<Player> {
        Arc::clone(&self.player)
    }

    pub fn state(&self) -> PlayerState {
        self.player.state()
    }

    pub fn position(&self) -> Duration {
        self.player.position()
    }

    pub fn current_format(&self) -> Option<AudioFormat> {
        self.player.current_format()
    }

    pub fn get_clock_stats(&self) -> Option<crate::audio::clock::ClockStats> {
        self.clock.lock().get_stats()
    }

    pub fn buffer_level(&self) -> f32 {
        let total = self.ring_buffer.capacity() as f32;
        let current = self.ring_buffer.len() as f32;
        if total > 0.0 {
            current / total
        } else {
            0.0
        }
    }
}

fn engine_worker(
    player: Arc<Player>,
    decoder: Arc<Mutex<Option<Decoder>>>,
    buffer_pool: Arc<Mutex<BufferPool>>,
    ring_buffer: SharedRingBuffer,
    clock: Arc<Mutex<AudioClock>>,
    clock_sync: ClockSync,
    config: AudioConfig,
    receiver: Receiver<EngineCommand>,
    event_sender: Sender<PlayerEvent>,
) {
    let mut running = true;
    let target_buffer_level = config.target_buffer_level;

    while running {
        if let Ok(cmd) = receiver.recv_timeout(Duration::from_millis(10)) {
            match cmd {
                EngineCommand::LoadTrack(path) => {
                    debug!("Loading track: {}", path);
                    player.stop().ok();
                    
                    match Decoder::new(&path) {
                        Ok(dec) => {
                            player.set_format(dec.format());
                            *decoder.lock() = Some(dec);
                            let _ = event_sender.send(PlayerEvent::TrackChanged(path));
                            info!("Track loaded successfully");
                        }
                        Err(e) => {
                            error!("Failed to load track: {}", e);
                            let _ = event_sender.send(PlayerEvent::Error(e.to_string()));
                        }
                    }
                }
                EngineCommand::Play => {
                    if let Err(e) = player.play() {
                        error!("Failed to play: {}", e);
                    } else {
                        clock.lock().start();
                    }
                }
                EngineCommand::Pause => {
                    if let Err(e) = player.pause() {
                        error!("Failed to pause: {}", e);
                    } else {
                        clock.lock().stop();
                    }
                }
                EngineCommand::Stop => {
                    if let Err(e) = player.stop() {
                        error!("Failed to stop: {}", e);
                    } else {
                        clock.lock().stop();
                        clock.lock().reset();
                        ring_buffer.clear();
                    }
                }
                EngineCommand::Seek(pos) => {
                    if let Err(e) = player.seek(pos) {
                        error!("Failed to seek: {}", e);
                    }
                }
                EngineCommand::SetVolume(vol) => {
                    debug!("Setting volume: {}", vol);
                }
                EngineCommand::SetFormat(format) => {
                    debug!("Setting format: {}", format);
                }
                EngineCommand::EnableDSP(enable) => {
                    debug!("DSP enabled: {}", enable);
                }
                EngineCommand::SetEQ(values) => {
                    debug!("Setting EQ: {:?}", values);
                }
                EngineCommand::Shutdown => {
                    running = false;
                    info!("Shutting down engine worker");
                }
            }
        }

        if player.is_playing() {
            let buffer_level = ring_buffer.len() as f32 / ring_buffer.capacity() as f32;

            if buffer_level < target_buffer_level {
                if let Some(dec) = decoder.lock().as_mut() {
                    match buffer_pool.lock().acquire() {
                        Some(mut buffer) => {
                            match dec.decode_next(&mut buffer) {
                                Ok(frames) => {
                                    if frames > 0 {
                                        let written = ring_buffer.write(buffer.data());
                                        debug!("Decoded {} frames, wrote {} bytes", frames, written);
                                    }
                                }
                                Err(e) => {
                                    warn!("Decode error: {}", e);
                                }
                            }
                            buffer_pool.lock().release(buffer);
                        }
                        None => {
                            warn!("No available buffers in pool");
                        }
                    }
                }
            }

            if let Some(stats) = clock.lock().get_stats() {
                if clock_sync.needs_correction(&stats) {
                    let correction = clock_sync.calculate_correction(&stats);
                    debug!("Clock drift detected: {:.2} ppm, correction: {:.6}", stats.drift_ppm, correction);
                }
            }
        }

        thread::sleep(Duration::from_millis(5));
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
