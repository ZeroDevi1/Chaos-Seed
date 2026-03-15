pub mod bili_video;
pub mod danmaku;
pub mod live_directory;
pub mod livestream;
pub mod llm;
pub mod lyrics;
pub mod music;
pub mod now_playing;
pub mod subtitle;
pub mod tts;
// voice_chat 依赖 TTS 推理，但 Python 运行时仅在真正调用时通过外部子进程触发。
pub mod voice_chat;

// Ensure Rustls has a selected CryptoProvider in builds where multiple providers are enabled.
// This avoids runtime panics like:
// "Could not automatically determine the process-level CryptoProvider..."
mod tls;
