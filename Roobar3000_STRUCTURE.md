# Roobar3000 - 高保真音频播放器架构文档

## 项目概述
高品质本地音频播放器，核心目标：**Bit-Perfect 音频还原** + **实时 GPU 加速可视化**

---

## 项目结构

Roobar3000/
├── LICENSE                         # MIT 许可证
├── README.md                       # 项目简介
├── CHANGELOG.md                    # 版本日志
├── CONTRIBUTING.md                 # 贡献指南
│
├── Cargo.toml                      # Rust 工作区根配置
├── Cargo.lock                      # 依赖锁文件
│
├── rust-core/                      # Rust 音频核心引擎
│   ├── Cargo.toml                  # 核心库配置
│   │
│   ├── src/
│   │   ├── main.rs                 # 后端服务主入口
│   │   ├── lib.rs                  # 库主入口
│   │   │
│   │   ├── audio/                  # 音频核心模块
│   │   │   ├── mod.rs
│   │   │   ├── engine.rs           # 音频引擎主控制
│   │   │   ├── player.rs           # 播放器状态机
│   │   │   ├── buffer_pool.rs      # 锁-free 音频缓冲区池（ringbuf）
│   │   │   ├── format.rs           # 音频格式定义（支持 16/24/32bit int, 32/64bit float）
│   │   │   └── clock.rs            # 设备时钟同步与抖动控制
│   │   │
│   │   ├── decoder/                # 音频解码器
│   │   │   ├── mod.rs
│   │   │   ├── symphonia_backend.rs # 统一使用 Symphonia（MP3/FLAC/WAV/OGG/AAC）
│   │   │   ├── resampler.rs        # 重采样处理器（关键质量路径）
│   │   │   └── stream.rs           # 音频流抽象
│   │   │
│   │   ├── output/                 # 音频输出系统（精简后端）
│   │   │   ├── mod.rs
│   │   │   ├── backend.rs          # 输出后端 Trait
│   │   │   ├── device.rs           # 设备抽象与独占模式管理
│   │   │   ├── wasapi.rs           # Windows WASAPI 独占模式（Bit-Perfect）
│   │   │   ├── coreaudio.rs        # macOS CoreAudio 独占模式
│   │   │   └── bitperfect.rs       # Bit-Perfect 验证与自动采样率匹配
│   │   │
│   │   ├── dsp/                    # 数字信号处理（精简版）
│   │   │   ├── mod.rs
│   │   │   ├── processor.rs        # DSP 处理器 Trait
│   │   │   ├── resampler_engine.rs # rubato 集成（Sinc/VHQ 模式）
│   │   │   └── eq.rs               # 10段参数均衡器（PEQ）
│   │   │
│   │   ├── library/                # 音乐库管理
│   │   │   ├── mod.rs
│   │   │   ├── scanner.rs          # 快速增量扫描
│   │   │   ├── metadata.rs         # 元数据提取（lofty-rs）
│   │   │   ├── database.rs         # SQLite（rusqlite）
│   │   │   ├── models.rs           # 数据模型
│   │   │   └── watch.rs            # 文件夹监视（notify）
│   │   │
│   │   ├── ipc/                    # 进程间通信
│   │   │   ├── mod.rs
│   │   │   ├── server.rs           # WebSocket 服务器（tokio-tungstenite）
│   │   │   ├── protocol.rs         # JSON-RPC 协议定义
│   │   │   └── handlers.rs         # 消息处理器
│   │   │
│   │   ├── config/                 # 配置管理
│   │   │   ├── mod.rs
│   │   │   ├── manager.rs          # 配置管理器
│   │   │   ├── schema.rs           # 配置验证（jsonschema）
│   │   │   └── audio.rs            # 音频路径配置
│   │   │
│   │   └── utils/                  # 工具模块
│   │       ├── mod.rs
│   │       ├── error.rs            # 统一错误类型（thiserror）
│   │       ├── logging.rs          # tracing 日志
│   │       ├── metrics.rs          # 性能指标（缓冲区欠载/抖动监控）
│   │       └── cache.rs            # 元数据缓存
│   │
│   └── tests/                      # Rust 测试
│       ├── engine_tests.rs
│       ├── resampler_tests.rs      # 关键：验证 rubato 输出质量
│       └── integration_tests.rs
│
├── tauri-frontend/                 # Tauri + Web 前端
│   ├── Cargo.toml                  # Tauri 配置
│   ├── tauri.conf.json             # Tauri 应用配置
│   ├── package.json                # Node.js 依赖
│   ├── index.html                  # 入口 HTML
│   │
│   ├── src/                        # TypeScript/React 源码
│   │   ├── main.tsx                # 应用入口
│   │   ├── App.tsx                 # 根组件
│   │   │
│   │   ├── components/             # UI 组件
│   │   │   ├── PlayerControls.tsx  # 播放控制
│   │   │   ├── SpectrumAnalyzer.tsx # WebGPU 频谱分析器
│   │   │   ├── WaveformDisplay.tsx  # 波形显示（WebGL）
│   │   │   ├── LibraryTree.tsx     # 音乐库树
│   │   │   └── EQPanel.tsx         # 均衡器面板
│   │   │
│   │   ├── hooks/                  # React Hooks
│   │   │   ├── useWebSocket.ts     # WebSocket 连接管理
│   │   │   ├── useAudioEngine.ts   # 音频引擎状态
│   │   │   └── useSpectrum.ts      # 频谱数据流
│   │   │
│   │   ├── workers/                # Web Workers
│   │   │   └── fft.worker.ts       # FFT 计算卸载
│   │   │
│   │   └── styles/                 # CSS/Tailwind
│   │       └── global.css
│   │
│   ├── src-tauri/                  # Tauri Rust 端（与 rust-core 通信）
│   │   ├── Cargo.toml
│   │   └── src/main.rs             # 启动 rust-core 并桥接 WebView
│   │
│   └── public/                     # 静态资源
│       ├── icons/
│       └── fonts/
│
├── configs/                        # 默认配置
│   └── default.toml
│
├── docs/                           # 文档
│   ├── architecture.md
│   ├── bitperfect-guide.md         # Bit-Perfect 配置指南
│   └── api.md
│
└── scripts/                        # 构建脚本
    ├── build.sh
    └── dev.sh

    技术栈详情
后端 (Rust) - 音质优先架构
表格
复制
组件	技术选型	音质考量
解码	symphonia	纯 Rust 实现，避免 FFI 开销，支持所有主流格式
重采样	rubato (Sinc/VHQ)	关键质量路径，使用 Sinc 插值算法，信噪比 > 144dB 
输出	cpal + 独占模式	WASAPI/CoreAudio 独占模式，绕过系统混音器
时钟	硬件时钟同步	自动匹配文件采样率与设备原生采样率，避免 SRC
缓冲区	ringbuf 无锁队列	零拷贝管道，分离解码/DSP/输出线程
IPC	tokio-tungstenite	异步 WebSocket，传输频谱数据/控制命令
Bit-Perfect 实现路径
独占模式：请求音频设备独占访问，阻止系统混音
格式直通：解码后数据直接送至设备，避免格式转换
采样率匹配：自动切换设备采样率匹配音频文件（如 44.1kHz → 48kHz 由 rubato 高品质重采样）
整数路径：当设备支持时，保持 24/32bit 整数传输，避免 FP32 转换
前端 (Tauri + Web) - GPU 加速可视化
表格
复制
技术	用途	性能特点
Tauri	桌面壳	Rust 后端，WebView2 渲染，包体 < 5MB
React + TypeScript	UI 框架	组件化，状态管理清晰
WebGPU / WebGL	频谱/波形渲染	GPU 加速 FFT 可视化，60fps 流畅 
Web Workers	FFT 计算	卸载主线程，避免音频卡顿
Tailwind CSS	样式	快速开发，支持深色模式
频谱分析实现
数据流：rust-core 通过 WebSocket 实时发送 PCM 数据块（降低采样率后的预览流，不影响主音频路径）
FFT 处理：Web Worker 中执行 fft.js 或 WebAssembly FFT，支持 2048-8192 点
渲染：WebGL 绘制频谱条/波形，支持平滑插值和峰值保持
核心特性实现
1. 高品质重采样（音质关键）
使用 rubato 库配置：
rust
复制
// 异步 Sinc 重采样，VHQ 模式
let params = SincInterpolationParameters {
    sinc_len: 256,           // 高长度 = 更好抗混叠
    f_cutoff: 0.95,          // 保留 95% 带宽
    interpolation: SincInterpolationType::Cubic, // 三次插值
    oversampling_factor: 128, // FFT 过采样
    window: WindowFunction::BlackmanHarris2,
};
延迟：约 20ms（VHQ 模式），可接受范围 
质量：接近理论完美重采样，优于系统默认 SRC
2. Bit-Perfect 输出
独占模式检测：自动检测设备是否支持独占模式，回退到共享模式时警告
采样率自动切换：播放 96kHz 文件时自动切换 DAC 到 96kHz
DSD 直通（扩展）：未来支持 DoP（DSD over PCM）传输
3. 实时 DSP（精简但专业）
10段参数均衡器：频率、Q值、增益可调，支持预设保存
处理精度：64bit 浮点内部处理，避免量化误差
旁通模式：可完全关闭 DSP，确保纯直通路径
4. GPU 加速可视化
实时频谱：1024 频段，30fps，GPU 渲染
波形显示：支持缩放和平移，显示解码后原始波形
性能隔离：可视化使用独立低优先级数据流，不影响音频线程
关键依赖版本
toml
复制
# rust-core/Cargo.toml
[dependencies]
symphonia = { version = "0.5", features = ["all"] }
rubato = "0.15"                    # 高品质重采样
cpal = "0.15"                      # 音频 I/O
ringbuf = "0.4"                    # 无锁缓冲区
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.21"         # WebSocket
rusqlite = { version = "0.31", features = ["bundled"] }
lofty = "0.21"                     # 元数据读取
serde = { version = "1", features = ["derive"] }
config = "0.14"
tracing = "0.1"
thiserror = "1.0"
JSON
复制
// tauri-frontend/package.json
{
  "dependencies": {
    "react": "^18.2.0",
    "@tauri-apps/api": "^1.5.0",
    "three": "^0.160.0",          // 3D 可视化备选
    "fft-js": "^0.0.12"           // 或 wasm-bindgen FFT
  },
  "devDependencies": {
    "@tauri-apps/cli": "^1.5.0",
    "typescript": "^5.3.0",
    "tailwindcss": "^3.4.0",
    "vite": "^5.0.0"
  }
}
开发阶段规划
Phase 1: 核心播放（2-3个月）
[ ] WASAPI 独占模式 Bit-Perfect 输出
[ ] Symphonia 解码 MP3/FLAC/WAV
[ ] rubato 重采样集成（44.1↔48kHz 测试）
[ ] Tauri 基础界面 + 播放控制
Phase 2: 可视化（1-2个月）
[ ] WebGL 频谱分析器（2048点 FFT）
[ ] 波形显示与缩放
[ ] 实时性能优化（Web Workers）
Phase 3: 音乐库（2个月）
[ ] 增量扫描 + SQLite 元数据缓存
[ ] 播放列表管理
[ ] 标签编辑器（lofty）
Phase 4: 高级功能（2个月）
[ ] 10段参数均衡器
[ ] macOS CoreAudio 支持
[ ] DSD 直通（DoP）
质量保证
重采样测试：对比 SoX VHQ 输出，确保 rubato 差异 < -144dB THD+N
Bit-Perfect 验证：循环测试（输出接输入），比对 MD5 哈希
延迟测试：全链路延迟 < 50ms（解码→DSP→输出）
可视化性能：4K 分辨率下频谱渲染 CPU 占用 < 5%
设计哲学：所有非必要处理均可旁通，确保从文件到 DAC 的最短、最高保真路径。