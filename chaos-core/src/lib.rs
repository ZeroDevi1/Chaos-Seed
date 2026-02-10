pub mod danmaku;
pub mod livestream;
pub mod now_playing;
pub mod lyrics;
pub mod subtitle;

// Ensure Rustls has a selected CryptoProvider in builds where multiple providers are enabled.
// This avoids runtime panics like:
// "Could not automatically determine the process-level CryptoProvider..."
mod tls;
