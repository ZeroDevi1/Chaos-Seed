#[test]
fn windows_icon_resource_uses_id_1() {
    let rc = std::fs::read_to_string("resources/windows/app.rc")
        .expect("resources/windows/app.rc should exist");
    assert!(
        rc.contains("1 ICON"),
        "app.rc should define the application icon as resource ID 1"
    );
}

#[test]
fn windows_icon_file_exists() {
    let meta =
        std::fs::metadata("resources/windows/app.ico").expect("resources/windows/app.ico exists");
    assert!(
        meta.len() > 0,
        "resources/windows/app.ico should not be empty"
    );
}
