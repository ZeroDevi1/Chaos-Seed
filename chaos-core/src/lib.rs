pub mod danmaku;
pub mod bili_video;
pub mod live_directory;
pub mod livestream;
pub mod lyrics;
pub mod llm;
pub mod music;
pub mod now_playing;
pub mod subtitle;
pub mod tts;
pub mod voice_chat;

// Ensure Rustls has a selected CryptoProvider in builds where multiple providers are enabled.
// This avoids runtime panics like:
// "Could not automatically determine the process-level CryptoProvider..."
mod tls;
