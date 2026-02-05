use crate::audio::player::Player;
use crate::ipc::protocol::{Request, Response, PlayerState as ProtocolPlayerState, AudioFormatInfo, DeviceInfo, MetricsInfo};
use crossbeam_channel::Sender;
use serde_json::json;
use tracing::debug;
use std::sync::Arc;

pub struct MessageHandler {
    command_sender: Sender<crate::audio::engine::EngineCommand>,
    player: Arc<Player>,
}

impl MessageHandler {
    pub fn new(command_sender: Sender<crate::audio::engine::EngineCommand>, player: Arc<Player>) -> Self {
        Self {
            command_sender,
            player,
        }
    }

    pub fn handle_request(&self, request: Request) -> Response {
        match request {
            Request::Play => self.handle_play(),
            Request::Pause => self.handle_pause(),
            Request::Stop => self.handle_stop(),
            Request::Seek { position } => self.handle_seek(position),
            Request::LoadTrack { path } => self.handle_load_track(path),
            Request::SetVolume { volume } => self.handle_set_volume(volume),
            Request::GetState => self.handle_get_state(),
            Request::GetPosition => self.handle_get_position(),
            Request::GetFormat => self.handle_get_format(),
            Request::SetEQ { bands } => self.handle_set_eq(bands),
            Request::EnableDSP { enabled } => self.handle_enable_dsp(enabled),
            Request::GetDevices => self.handle_get_devices(),
            Request::SetDevice { index } => self.handle_set_device(index),
            Request::GetVolume => self.handle_get_volume(),
            Request::GetMetrics => self.handle_get_metrics(),
            _ => Response::error(None, "Method not implemented".into()),
        }
    }

    fn handle_play(&self) -> Response {
        debug!("Handling play request");
        match self.player.play() {
            Ok(_) => Response::success(None, json!({ "status": "playing" })),
            Err(e) => Response::error(None, e.to_string()),
        }
    }

    fn handle_pause(&self) -> Response {
        debug!("Handling pause request");
        match self.player.pause() {
            Ok(_) => Response::success(None, json!({ "status": "paused" })),
            Err(e) => Response::error(None, e.to_string()),
        }
    }

    fn handle_stop(&self) -> Response {
        debug!("Handling stop request");
        match self.player.stop() {
            Ok(_) => Response::success(None, json!({ "status": "stopped" })),
            Err(e) => Response::error(None, e.to_string()),
        }
    }

    fn handle_seek(&self, position: u64) -> Response {
        debug!("Handling seek request: {}", position);
        match self.player.seek(std::time::Duration::from_secs(position)) {
            Ok(_) => Response::success(None, json!({ "position": position })),
            Err(e) => Response::error(None, e.to_string()),
        }
    }

    fn handle_load_track(&self, path: String) -> Response {
        debug!("Handling load track request: {}", path);
        let _ = self.command_sender.send(crate::audio::engine::EngineCommand::LoadTrack(path.clone()));
        Response::success(None, json!({ "path": path }))
    }

    fn handle_set_volume(&self, volume: f32) -> Response {
        debug!("Handling set volume request: {}", volume);
        let _ = self.command_sender.send(crate::audio::engine::EngineCommand::SetVolume(volume));
        Response::success(None, json!({ "volume": volume }))
    }

    fn handle_get_state(&self) -> Response {
        debug!("Handling get state request");
        let state = self.player.state();
        let position = self.player.position().as_secs();
        let volume = 1.0;
        let format = self.player.current_format();

        let protocol_state = ProtocolPlayerState {
            state: format!("{:?}", state),
            position,
            duration: None,
            track: None,
            volume,
            format: format.map(|f| AudioFormatInfo {
                sample_rate: f.sample_rate,
                channels: f.channels,
                sample_format: format!("{:?}", f.sample_format),
                bit_depth: None,
            }),
        };

        Response::success(None, json!(protocol_state))
    }

    fn handle_get_position(&self) -> Response {
        debug!("Handling get position request");
        let position = self.player.position().as_secs();
        Response::success(None, json!({ "position": position }))
    }

    fn handle_get_format(&self) -> Response {
        debug!("Handling get format request");
        match self.player.current_format() {
            Some(format) => {
                let info = AudioFormatInfo {
                    sample_rate: format.sample_rate,
                    channels: format.channels,
                    sample_format: format!("{:?}", format.sample_format),
                    bit_depth: None,
                };
                Response::success(None, json!(info))
            }
            None => Response::error(None, "No format available".into()),
        }
    }

    fn handle_set_eq(&self, bands: Vec<f32>) -> Response {
        debug!("Handling set EQ request: {:?}", bands);
        let _ = self.command_sender.send(crate::audio::engine::EngineCommand::SetEQ(bands.clone()));
        Response::success(None, json!({ "bands": bands }))
    }

    fn handle_enable_dsp(&self, enabled: bool) -> Response {
        debug!("Handling enable DSP request: {}", enabled);
        let _ = self.command_sender.send(crate::audio::engine::EngineCommand::EnableDSP(enabled));
        Response::success(None, json!({ "enabled": enabled }))
    }

    fn handle_get_devices(&self) -> Response {
        debug!("Handling get devices request");
        let devices: Vec<DeviceInfo> = Vec::new();
        Response::success(None, json!(devices))
    }

    fn handle_set_device(&self, index: usize) -> Response {
        debug!("Handling set device request: {}", index);
        Response::success(None, json!({ "device": index }))
    }

    fn handle_get_volume(&self) -> Response {
        debug!("Handling get volume request");
        Response::success(None, json!({ "volume": 1.0 }))
    }

    fn handle_get_metrics(&self) -> Response {
        debug!("Handling get metrics request");
        let metrics = MetricsInfo {
            buffer_underruns: 0,
            buffer_overruns: 0,
            decode_errors: 0,
            output_errors: 0,
            average_latency_ms: 0.0,
            peak_latency_ms: 0.0,
            jitter_ns: 0,
            clock_drift_ppm: 0.0,
            health_score: 100.0,
        };

        Response::success(None, json!(metrics))
    }
}
