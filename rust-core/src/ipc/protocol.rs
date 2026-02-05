use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Message {
    Request(Request),
    Response(Response),
    Notification(Notification),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum Request {
    #[serde(rename = "player.play")]
    Play,
    #[serde(rename = "player.pause")]
    Pause,
    #[serde(rename = "player.stop")]
    Stop,
    #[serde(rename = "player.seek")]
    Seek { position: u64 },
    #[serde(rename = "player.load")]
    LoadTrack { path: String },
    #[serde(rename = "player.set_volume")]
    SetVolume { volume: f32 },
    #[serde(rename = "player.get_state")]
    GetState,
    #[serde(rename = "player.get_position")]
    GetPosition,
    #[serde(rename = "player.get_format")]
    GetFormat,
    #[serde(rename = "player.set_eq")]
    SetEQ { bands: Vec<f32> },
    #[serde(rename = "player.enable_dsp")]
    EnableDSP { enabled: bool },
    #[serde(rename = "library.scan")]
    ScanLibrary { path: String },
    #[serde(rename = "library.get_tracks")]
    GetTracks { offset: usize, limit: usize },
    #[serde(rename = "library.search")]
    SearchTracks { query: String },
    #[serde(rename = "output.get_devices")]
    GetDevices,
    #[serde(rename = "output.set_device")]
    SetDevice { index: usize },
    #[serde(rename = "output.get_volume")]
    GetVolume,
    #[serde(rename = "config.get")]
    GetConfig,
    #[serde(rename = "config.set")]
    SetConfig { key: String, value: serde_json::Value },
    #[serde(rename = "metrics.get")]
    GetMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum Response {
    #[serde(rename = "success")]
    Success { id: Option<String>, result: serde_json::Value },
    #[serde(rename = "error")]
    Error { id: Option<String>, error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum Notification {
    #[serde(rename = "player.state_changed")]
    StateChanged { state: String },
    #[serde(rename = "player.track_changed")]
    TrackChanged { path: String },
    #[serde(rename = "player.position_changed")]
    PositionChanged { position: u64 },
    #[serde(rename = "player.buffer_underrun")]
    BufferUnderrun,
    #[serde(rename = "library.scan_progress")]
    ScanProgress { progress: f32, total: usize },
    #[serde(rename = "library.scan_complete")]
    ScanComplete { count: usize },
    #[serde(rename = "output.device_changed")]
    DeviceChanged { name: String },
    #[serde(rename = "config.changed")]
    ConfigChanged { key: String },
    #[serde(rename = "error")]
    Error { message: String },
}

impl Message {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl Request {
    pub fn to_message(self) -> Message {
        Message::Request(self)
    }
}

impl Response {
    pub fn success(id: Option<String>, result: serde_json::Value) -> Self {
        Self::Success { id, result }
    }

    pub fn error(id: Option<String>, error: String) -> Self {
        Self::Error { id, error }
    }

    pub fn to_message(self) -> Message {
        Message::Response(self)
    }
}

impl Notification {
    pub fn to_message(self) -> Message {
        Message::Notification(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub state: String,
    pub position: u64,
    pub duration: Option<u64>,
    pub track: Option<TrackInfo>,
    pub volume: f32,
    pub format: Option<AudioFormatInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<u64>,
    pub format: AudioFormatInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFormatInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
    pub bit_depth: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub index: usize,
    pub name: String,
    pub is_default: bool,
    pub max_channels: u16,
    pub max_sample_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsInfo {
    pub buffer_underruns: u64,
    pub buffer_overruns: u64,
    pub decode_errors: u64,
    pub output_errors: u64,
    pub average_latency_ms: f64,
    pub peak_latency_ms: f64,
    pub jitter_ns: u64,
    pub clock_drift_ppm: f64,
    pub health_score: f64,
}
