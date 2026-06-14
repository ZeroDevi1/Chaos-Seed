import Foundation
import CryptoKit

/// MD5 / Base64 等基础编解码，对齐 Rust `md5::compute` 与 `base64::engine::general_purpose::STANDARD`。
public enum Crypto {
    /// 计算字符串的 MD5，返回小写十六进制。
    /// 对齐 Rust `format!("{:x}", md5::compute(s))`。
    public static func md5Hex(_ s: String) -> String {
        let digest = Insecure.MD5.hash(data: Data(s.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    /// Base64 解码为字符串（UTF-8）。
    public static func base64DecodeToString(_ s: String) -> String? {
        guard let data = Data(base64Encoded: s) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// Base64 编码字符串。
    public static func base64Encode(_ s: String) -> String {
        Data(s.utf8).base64EncodedString()
    }
}
