use crate::audio::format::AudioFormat;
use crate::utils::error::{AudioError, Result};
use crossbeam_channel::{Sender, Receiver, unbounded};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
    Buffering,
    Error,
}

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    StateChanged(PlayerState),
    TrackChanged(String),
    PositionChanged(Duration),
    BufferUnderrun,
    Error(String),
}

pub struct Player {
    state: Arc<Mutex<PlayerState>>,
    event_sender: Sender<PlayerEvent>,
    event_receiver: Receiver<PlayerEvent>,
    current_format: Arc<Mutex<Option<AudioFormat>>>,
    position: Arc<Mutex<Duration>>,
}

impl Player {
    pub fn new() -> Self {
        let (event_sender, event_receiver) = unbounded();
        Self {
            state: Arc::new(Mutex::new(PlayerState::Stopped)),
            event_sender,
            event_receiver,
            current_format: Arc::new(Mutex::new(None)),
            position: Arc::new(Mutex::new(Duration::ZERO)),
        }
    }

    pub fn state(&self) -> PlayerState {
        *self.state.lock()
    }

    pub fn set_state(&self, new_state: PlayerState) {
        let old_state = *self.state.lock();
        if old_state != new_state {
            *self.state.lock() = new_state;
            let _ = self.event_sender.send(PlayerEvent::StateChanged(new_state));
        }
    }

    pub fn event_receiver(&self) -> &Receiver<PlayerEvent> {
        &self.event_receiver
    }

    pub fn current_format(&self) -> Option<AudioFormat> {
        *self.current_format.lock()
    }

    pub fn set_format(&self, format: AudioFormat) {
        *self.current_format.lock() = Some(format);
    }

    pub fn position(&self) -> Duration {
        *self.position.lock()
    }

    pub fn set_position(&self, pos: Duration) {
        *self.position.lock() = pos;
        let _ = self.event_sender.send(PlayerEvent::PositionChanged(pos));
    }

    pub fn play(&self) -> Result<()> {
        match self.state() {
            PlayerState::Stopped | PlayerState::Paused => {
                self.set_state(PlayerState::Playing);
                Ok(())
            }
            PlayerState::Playing => Ok(()),
            _ => Err(AudioError::InvalidState("Cannot play in current state".into())),
        }
    }

    pub fn pause(&self) -> Result<()> {
        match self.state() {
            PlayerState::Playing => {
                self.set_state(PlayerState::Paused);
                Ok(())
            }
            PlayerState::Paused => Ok(()),
            _ => Err(AudioError::InvalidState("Cannot pause in current state".into())),
        }
    }

    pub fn stop(&self) -> Result<()> {
        match self.state() {
            PlayerState::Playing | PlayerState::Paused | PlayerState::Buffering => {
                self.set_state(PlayerState::Stopped);
                self.set_position(Duration::ZERO);
                Ok(())
            }
            PlayerState::Stopped => Ok(()),
            _ => Err(AudioError::InvalidState("Cannot stop in current state".into())),
        }
    }

    pub fn seek(&self, position: Duration) -> Result<()> {
        self.set_position(position);
        Ok(())
    }

    pub fn is_playing(&self) -> bool {
        self.state() == PlayerState::Playing
    }

    pub fn is_paused(&self) -> bool {
        self.state() == PlayerState::Paused
    }

    pub fn is_stopped(&self) -> bool {
        self.state() == PlayerState::Stopped
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
