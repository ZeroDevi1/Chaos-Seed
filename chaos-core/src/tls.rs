use std::sync::Once;

static INIT: Once = Once::new();

pub fn ensure_rustls_provider() {
    INIT.call_once(|| {
        // If multiple crypto providers are enabled in the dependency graph (e.g. ring + aws-lc),
        // rustls requires the app to select one explicitly.
        //
        // Prefer aws-lc-rs (rustls default), and ignore errors if another part of the process
        // already picked.
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}
