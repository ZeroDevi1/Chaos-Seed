fn main() {
    slint_build::compile("ui/app.slint").expect("slint build failed");

    println!("cargo:rerun-if-changed=resources/windows/app.rc");
    println!("cargo:rerun-if-changed=resources/windows/app.ico");

    // Embed Windows resources (icon) when the *target* is Windows.
    // This is executed in the build-script on the host, so we inspect target cfg vars.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::path::Path::new("resources/windows/app.rc").exists()
    {
        let _ = embed_resource::compile("resources/windows/app.rc", embed_resource::NONE);
    }
}
