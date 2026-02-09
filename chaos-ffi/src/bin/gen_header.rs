use std::path::PathBuf;

fn main() {
    // Generate `include/chaos_ffi_bindings.h` using the crate-local `cbindgen.toml`.
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg_path = crate_dir.join("cbindgen.toml");
    let out_path = crate_dir.join("include").join("chaos_ffi_bindings.h");

    let cfg = cbindgen::Config::from_file(&cfg_path).unwrap_or_else(|e| {
        panic!(
            "failed to read cbindgen config at {}: {e}",
            cfg_path.display()
        )
    });

    std::fs::create_dir_all(out_path.parent().unwrap()).expect("create include dir");

    let header = cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(cfg)
        .generate()
        .expect("cbindgen generate");

    header.write_to_file(&out_path);
    eprintln!("generated {}", out_path.display());
}
