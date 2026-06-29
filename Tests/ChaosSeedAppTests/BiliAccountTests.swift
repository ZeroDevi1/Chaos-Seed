import XCTest
@testable import ChaosSeedApp

final class BiliAccountTests: XCTestCase {
    func testCombinedCookieAppendsMissingBuvid() {
        let cookie = BiliAccountStore.combinedCookie(
            loginCookie: "SESSDATA=abc; bili_jct=csrf",
            buvidCookie: "buvid3=b3; buvid4=b4;"
        )

        XCTAssertEqual(cookie, "SESSDATA=abc;bili_jct=csrf;buvid3=b3;buvid4=b4")
    }

    func testCombinedCookieDoesNotDuplicateBuvid() {
        let cookie = BiliAccountStore.combinedCookie(
            loginCookie: "SESSDATA=abc; buvid3=existing",
            buvidCookie: "buvid3=b3; buvid4=b4;"
        )

        XCTAssertEqual(cookie, "SESSDATA=abc;buvid3=existing;buvid4=b4")
    }

    func testCookieStringFromSetCookieHeader() {
        let cookie = BiliAccountStore.cookieString(from: [
            "Set-Cookie": "SESSDATA=abc; Path=/; Domain=.bilibili.com, bili_jct=csrf; Path=/; Domain=.bilibili.com",
        ])

        XCTAssertTrue(cookie.contains("SESSDATA=abc"))
        XCTAssertTrue(cookie.contains("bili_jct=csrf"))
    }
}
