import XCTest
@testable import ChaosSeedApp

/// BiliLive 解析对照测试：基于 chaos-core `tests/livestream_bili_live_mock.rs` 的 JSON fixture，
/// 验证 `biliParseRoomPlayInfoValue` / `pickBestCodec` 的输出与 Rust 一致。
///
/// 对照点：
/// 1. 选中的 codec branch（avc / http_stream / flv 优先）
/// 2. URL 绑定逻辑（current_qn != requested_qn 时仍按 requested_qn 绑定）
/// 3. backup_urls 顺序（MBGA 排序后 mirror 优先）
/// 4. variant 列表完整性
final class BiliParseFixturesTests: XCTestCase {

    // MARK: - pickBestCodec

    /// 验证优先选择 `http_stream + flv + avc` 的 codec。
    func testPickBestCodec_prefersHttpStreamFlvAvc() throws {
        let jsonStr = #"""
        [
            {
                "protocol_name": "http_hls",
                "format": [{
                    "format_name": "fmp4",
                    "codec": [{
                        "codec_name": "hevc",
                        "current_qn": 1000,
                        "accept_qn": [1000],
                        "base_url": "/hls.fmp4",
                        "url_info": [{"host": "https://cdn1.com", "extra": ""}]
                    }]
                }]
            },
            {
                "protocol_name": "http_stream",
                "format": [{
                    "format_name": "flv",
                    "codec": [{
                        "codec_name": "avc",
                        "current_qn": 2000,
                        "accept_qn": [1000, 2000],
                        "base_url": "/stream.flv",
                        "url_info": [{"host": "https://cdn2.com", "extra": ""}]
                    }]
                }]
            }
        ]
        """#
        let streams = try JSONValue(parsing: jsonStr).asArray ?? []

        let best = pickBestCodec(streams)
        XCTAssertNotNil(best, "应选出最佳 codec")
        XCTAssertEqual(best?.pointer("/codec_name")?.asString, "avc")
        XCTAssertEqual(best?.pointer("/current_qn")?.asInt64, 2000)
    }

    /// 验证缺少 base_url 或 url_info 的 codec 被跳过。
    func testPickBestCodec_skipsMissingUrlInfo() throws {
        let jsonStr = #"""
        [{
            "protocol_name": "http_stream",
            "format": [{
                "format_name": "flv",
                "codec": [
                    {
                        "codec_name": "avc",
                        "current_qn": 1000,
                        "accept_qn": [1000],
                        "base_url": "",
                        "url_info": []
                    },
                    {
                        "codec_name": "avc",
                        "current_qn": 2000,
                        "accept_qn": [2000],
                        "base_url": "/valid.flv",
                        "url_info": [{"host": "https://cdn.com", "extra": "?t=1"}]
                    }
                ]
            }]
        }]
        """#
        let streams = try JSONValue(parsing: jsonStr).asArray ?? []

        let best = pickBestCodec(streams)
        XCTAssertNotNil(best)
        XCTAssertEqual(best?.pointer("/current_qn")?.asInt64, 2000)
    }

    // MARK: - parseRoomPlayInfoValue（对照 Rust fixture）

    /// 对照 Rust `decode_manifest_room_play_info_ok`：正常解析，URL 绑定到 current_qn。
    func testParseRoomPlayInfoValue_normalBinding() throws {
        let json = try JSONValue(parsing: Self.fixtureNormal)

        let vars = try biliParseRoomPlayInfoValue(json, requestedQn: nil)
        XCTAssertEqual(vars.count, 2, "应解析出 2 个清晰度")

        // 原画 (qn=2000)：current_qn 匹配，应绑定 URL。
        let high = vars.first(where: { $0.quality == 2000 })
        XCTAssertNotNil(high)
        XCTAssertNotNil(high?.url, "current_qn=2000 应绑定 URL")
        // MBGA 排序后 mirror 优先。
        XCTAssertTrue(high?.url?.contains("up-mirror.bilivideo.com") == true, "MBGA 排序后 mirror CDN 应优先")
        XCTAssertEqual(high?.backupUrls.count, 1, "应有一个备用 URL")

        // 高清 (qn=1000)：current_qn 不匹配，不应绑定。
        let low = vars.first(where: { $0.quality == 1000 })
        XCTAssertNotNil(low)
        XCTAssertNil(low?.url, "qn=1000 不匹配 current_qn 不应绑定 URL")
    }

    /// 对照 Rust：请求指定 qn 时，即使 current_qn 异常也按 requested_qn 绑定。
    func testParseRoomPlayInfoValue_bindByRequestedQn() throws {
        let json = try JSONValue(parsing: Self.fixtureCurrentQnMismatch)

        // 请求 qn=10000。
        let vars = try biliParseRoomPlayInfoValue(json, requestedQn: 10000)
        XCTAssertEqual(vars.count, 2)

        // qn=10000 应绑定 URL（尽管 current_qn=250）。
        let high = vars.first(where: { $0.quality == 10000 })
        XCTAssertNotNil(high, "应包含 qn=10000 的 variant")
        XCTAssertNotNil(high?.url, "请求 qn=10000 时即使 current_qn=250 也应绑定")
        XCTAssertEqual(high?.backupUrls.count, 1)

        // qn=250：不应绑定（不是 requested_qn）。
        let low = vars.first(where: { $0.quality == 250 })
        XCTAssertNotNil(low)
        XCTAssertNil(low?.url)
    }

    /// 对齐 main：URL 查询参数不能作为可靠画质证据，仍按 requested_qn 绑定。
    func testParseRoomPlayInfoValue_keepsRequestedQualityWhenURLQnDiffers() throws {
        let json = try JSONValue(parsing: Self.fixtureExplicitQnFallback)

        let vars = try biliParseRoomPlayInfoValue(json, requestedQn: 10000)
        let high = vars.first(where: { $0.quality == 10000 })
        XCTAssertNotNil(high)
        XCTAssertTrue(high?.url?.contains("qn=250") == true)
    }

    func testParseRoomPlayInfoValue_keepsAllCDNBackupsForRequestedQuality() throws {
        let json = try JSONValue(parsing: Self.fixtureMixedURLQualities)

        let vars = try biliParseRoomPlayInfoValue(json, requestedQn: 10000)
        let high = try XCTUnwrap(vars.first(where: { $0.quality == 10000 }))
        XCTAssertEqual(high.allURLs.count, 2)
        XCTAssertTrue(high.allURLs.contains(where: { $0.contains("qn=250") }))
        XCTAssertTrue(high.allURLs.contains(where: { $0.contains("qn=10000") }))
    }

    func testBiliActualQnDiagnosticTextUsesPrimaryURL() throws {
        let variant = StreamVariant(
            id: "bili_live:10000:原画",
            label: "原画",
            quality: 10000,
            url: "https://example.com/live_2500.flv?expires=1&qn=250&expected_qn=250",
            backupUrls: ["https://example.com/live_10000.flv?qn=10000"]
        )

        XCTAssertEqual(variant.biliActualQn, 250)
        XCTAssertEqual(variant.biliActualQualityText, "720P 超清 (qn=250)")
        XCTAssertEqual(variant.biliQualityDiagnosticText, "请求 原画 (qn=10000)，实际 720P 超清 (qn=250)")
    }

    func testBiliActualQnFallsBackToExpectedQn() throws {
        let variant = StreamVariant(
            id: "bili_live:10000:原画",
            label: "原画",
            quality: 10000,
            url: "https://example.com/live_2500.flv?expires=1&expected_qn=250"
        )

        XCTAssertEqual(variant.biliActualQn, 250)
    }

    /// 对照 Rust：accept_qn 过滤掉不支持的清晰度。
    func testParseRoomPlayInfoValue_filtersByAcceptQn() throws {
        let json = try JSONValue(parsing: Self.fixtureAcceptQnFilter)

        let vars = try biliParseRoomPlayInfoValue(json, requestedQn: nil)
        // g_qn_desc 包含 qn=4000，但 accept_qn 只有 [1000, 2000] → qn=4000 应被过滤。
        XCTAssertEqual(vars.count, 2)
        XCTAssertFalse(vars.contains(where: { $0.quality == 4000 }), "accept_qn 中不含 4000，应被过滤")
    }

    /// 对照 Rust：encrypted=true + pwd_verified=false → throw NeedPassword。
    func testParseRoomPlayInfoValue_needPassword() throws {
        let jsonStr = #"""
        {"code":0,"data":{"encrypted":true,"pwd_verified":false}}
        """#
        let json = try JSONValue(parsing: jsonStr)

        XCTAssertThrowsError(try biliParseRoomPlayInfoValue(json, requestedQn: nil)) { error in
            guard case LiveKitError.needPassword = error else {
                XCTFail("expected needPassword, got \(error)")
                return
            }
        }
    }

    // MARK: - variant ID 格式

    func testVariantIdFormat() {
        let id = biliMakeVariantId(qn: 2000, label: "原画")
        XCTAssertEqual(id, "bili_live:2000:原画")
    }

    func testBuiltinPlaybackSourcePrefersHLSAndSupportsHTTPFLV() throws {
        let hls = "https://example.com/live/index.m3u8?token=1"
        let withHLS = StreamVariant(
            id: "bili_live:10000:原画",
            label: "原画",
            quality: 10000,
            url: "https://example.com/live.flv?token=1",
            backupUrls: [hls]
        )
        XCTAssertEqual(withHLS.builtinPlaybackURL?.absoluteString, hls)
        XCTAssertEqual(withHLS.builtinPlaybackSource?.engine, .avFoundation)

        let flvOnly = StreamVariant(
            id: "huya:0:原画",
            label: "原画",
            quality: 10000,
            url: "https://example.com/live.flv?token=1"
        )
        XCTAssertNil(flvOnly.builtinPlaybackURL)
        XCTAssertEqual(flvOnly.webFLVPlaybackURL?.absoluteString, "https://example.com/live.flv?token=1")
        XCTAssertEqual(flvOnly.builtinPlaybackSource?.engine, .libMPV)
        XCTAssertTrue(flvOnly.needsBuiltinResolution(for: .huya))
        XCTAssertFalse(flvOnly.needsBuiltinResolution(for: .douyu))
        XCTAssertFalse(withHLS.needsBuiltinResolution(for: .biliLive))

        let p2pOnly = StreamVariant(
            id: "douyu:0:原画",
            label: "原画",
            quality: 10000,
            url: "https://example.com/live.xs?token=1"
        )
        XCTAssertNil(p2pOnly.builtinPlaybackURL)
        XCTAssertNil(p2pOnly.webFLVPlaybackURL)
        XCTAssertNil(p2pOnly.builtinPlaybackSource)
    }

    // MARK: - JSON fixture 字符串

    /// 正常 fixture：current_qn=2000，accept_qn=[1000,2000]。
    /// 对照 Rust `decode_manifest_room_play_info_ok`。
    private static let fixtureNormal = #"""
    {
        "code": 0,
        "data": {
            "encrypted": false,
            "pwd_verified": true,
            "playurl_info": {
                "playurl": {
                    "g_qn_desc": [
                        {"qn": 1000, "desc": "高清"},
                        {"qn": 2000, "desc": "原画"}
                    ],
                    "stream": [{
                        "protocol_name": "http_stream",
                        "format": [{
                            "format_name": "flv",
                            "codec": [{
                                "codec_name": "avc",
                                "current_qn": 2000,
                                "accept_qn": [1000, 2000],
                                "base_url": "/live-bvc/xx.flv",
                                "url_info": [
                                    {"host": "https://foo.mcdn.bilivideo.cn", "extra": "?x=1"},
                                    {"host": "https://up-mirror.bilivideo.com", "extra": "?x=1"}
                                ]
                            }]
                        }]
                    }]
                }
            }
        }
    }
    """#

    /// current_qn=250 但 accept_qn 包含 10000 的异常 fixture。
    /// 对照 Rust `decode_manifest_room_play_info_ignores_qn_fallback_to_playurl`。
    private static let fixtureCurrentQnMismatch = #"""
    {
        "code": 0,
        "data": {
            "encrypted": false,
            "pwd_verified": true,
            "playurl_info": {
                "playurl": {
                    "g_qn_desc": [
                        {"qn": 250, "desc": "超清"},
                        {"qn": 10000, "desc": "原画"}
                    ],
                    "stream": [{
                        "protocol_name": "http_stream",
                        "format": [{
                            "format_name": "flv",
                            "codec": [{
                                "codec_name": "avc",
                                "current_qn": 250,
                                "accept_qn": [250, 10000],
                                "base_url": "/live-bvc/hi_10000.flv",
                                "url_info": [
                                    {"host": "https://up-mirror.bilivideo.com", "extra": "?x=1"},
                                    {"host": "https://cdn-bak.example.com", "extra": "?x=1"}
                                ]
                            }]
                        }]
                    }]
                }
            }
        }
    }
    """#

    /// 真实房间常见回退：请求原画，但 current_qn 与 URL 查询参数都明确为 250。
    private static let fixtureExplicitQnFallback = #"""
    {
        "code": 0,
        "data": {
            "encrypted": false,
            "pwd_verified": true,
            "playurl_info": {
                "playurl": {
                    "g_qn_desc": [
                        {"qn": 250, "desc": "超清"},
                        {"qn": 10000, "desc": "原画"}
                    ],
                    "stream": [{
                        "protocol_name": "http_hls",
                        "format": [{
                            "format_name": "ts",
                            "codec": [{
                                "codec_name": "avc",
                                "current_qn": 250,
                                "accept_qn": [250, 10000],
                                "base_url": "/live_2500.m3u8",
                                "url_info": [{
                                    "host": "https://example.com",
                                    "extra": "?qn=250&token=1"
                                }]
                            }]
                        }]
                    }]
                }
            }
        }
    }
    """#

    private static let fixtureMixedURLQualities = #"""
    {
        "code": 0,
        "data": {
            "encrypted": false,
            "pwd_verified": true,
            "playurl_info": {
                "playurl": {
                    "g_qn_desc": [
                        {"qn": 250, "desc": "超清"},
                        {"qn": 10000, "desc": "原画"}
                    ],
                    "stream": [{
                        "protocol_name": "http_hls",
                        "format": [{
                            "format_name": "ts",
                            "codec": [{
                                "codec_name": "avc",
                                "current_qn": 250,
                                "accept_qn": [250, 10000],
                                "base_url": "/live.m3u8",
                                "url_info": [
                                    {"host": "https://low.example.com", "extra": "?qn=250"},
                                    {"host": "https://high.example.com", "extra": "?qn=10000"}
                                ]
                            }]
                        }]
                    }]
                }
            }
        }
    }
    """#

    /// accept_qn 过滤 fixture：g_qn_desc 含 qn=4000 但 accept_qn 不含。
    private static let fixtureAcceptQnFilter = #"""
    {
        "code": 0,
        "data": {
            "encrypted": false,
            "pwd_verified": true,
            "playurl_info": {
                "playurl": {
                    "g_qn_desc": [
                        {"qn": 1000, "desc": "高清"},
                        {"qn": 2000, "desc": "原画"},
                        {"qn": 4000, "desc": "杜比"}
                    ],
                    "stream": [{
                        "protocol_name": "http_stream",
                        "format": [{
                            "format_name": "flv",
                            "codec": [{
                                "codec_name": "avc",
                                "current_qn": 2000,
                                "accept_qn": [1000, 2000],
                                "base_url": "/live.flv",
                                "url_info": [
                                    {"host": "https://up-mirror.bilivideo.com", "extra": ""}
                                ]
                            }]
                        }]
                    }]
                }
            }
        }
    }
    """#
}
