/// CLI 集成测试
/// 注意：这些测试需要在有网络连接的环境中运行
use std::process::Command;

#[test]
fn test_cli_help() {
    let output = Command::new("cargo")
        .args(["run", "-p", "chaos-cli", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("chaos-cli"));
    assert!(stdout.contains("resolve"));
    assert!(stdout.contains("danmaku"));
    assert!(stdout.contains("play"));
    assert!(stdout.contains("tui"));
}

#[test]
fn test_resolve_help() {
    let output = Command::new("cargo")
        .args(["run", "-p", "chaos-cli", "--", "resolve", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("format"));
    assert!(stdout.contains("quality"));
}

#[test]
fn test_danmaku_help() {
    let output = Command::new("cargo")
        .args(["run", "-p", "chaos-cli", "--", "danmaku", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("output"));
    assert!(stdout.contains("filter"));
}

#[test]
fn test_play_help() {
    let output = Command::new("cargo")
        .args(["run", "-p", "chaos-cli", "--", "play", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("player"));
    assert!(stdout.contains("quality"));
}
