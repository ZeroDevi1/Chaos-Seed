// swift-tools-version: 5.10
// Chaos Seed macOS —— 直播源解析原生应用
// 设计文档：docs/superpowers/specs/2026-06-13-chaos-seed-macos-design.md
// 高保真原型：designs/ChaosSeed-macOS/
// Rust 协议蓝本：github.com/ZeroDevi1/Chaos-Seed main 分支 chaos-core

import PackageDescription

let package = Package(
    name: "ChaosSeed",
    platforms: [
        .macOS(.v14),
    ],
    products: [
        .executable(name: "ChaosSeed", targets: ["ChaosSeedApp"]),
    ],
    targets: [
        .executableTarget(
            name: "ChaosSeedApp",
            path: "Sources/ChaosSeedApp",
            resources: [
                .copy("Resources"),
            ]
        ),
        .testTarget(
            name: "ChaosSeedAppTests",
            dependencies: ["ChaosSeedApp"],
            path: "Tests/ChaosSeedAppTests"
        ),
    ]
)
