pub mod danmaku;
pub mod bili_video;
pub mod live_directory;
pub mod livestream;
pub mod lyrics;
pub mod music;
pub mod now_playing;
pub mod subtitle;

// Ensure Rustls has a selected CryptoProvider in builds where multiple providers are enabled.
// This avoids runtime panics like:
// "Could not automatically determine the process-level CryptoProvider..."
mod tls;
