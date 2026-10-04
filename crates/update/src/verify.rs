//! 完整性（sha256）与来源（minisign Ed25519）校验。
//!
//! 签名与公钥都沿用 Tauri 的外层 base64 编码：发布机用 `tauri signer sign` 签名，
//! 客户端在这里解开外层后交给 `minisign-verify`。

use crate::UpdateError;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};

/// 用 `public_key`（外层 base64）验证 `data` 的 `signature`（外层 base64）。
pub(crate) fn verify_signature(
    data: &[u8],
    signature: &str,
    public_key: &str,
) -> Result<(), UpdateError> {
    let key = decode_outer(public_key)
        .and_then(|text| PublicKey::decode(&text).map_err(|e| e.to_string()))
        .map_err(|e| UpdateError::Signature(format!("公钥无效：{e}")))?;
    let signature = decode_outer(signature)
        .and_then(|text| Signature::decode(&text).map_err(|e| e.to_string()))
        .map_err(|e| UpdateError::Signature(format!("签名格式无效：{e}")))?;
    // 只接受预哈希签名（`ED`）；`tauri signer sign` 产出的就是这种。
    key.verify(data, &signature, false)
        .map_err(|e| UpdateError::Signature(e.to_string()))
}

fn decode_outer(value: &str) -> Result<String, String> {
    let bytes = STANDARD
        .decode(value.trim())
        .map_err(|e| format!("base64 无效：{e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("不是 UTF-8：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 测试专用的一次性密钥（私钥未保存），签名对象为 FIXTURE。
    const TEST_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDlFOTBDNTlGMzE3MEI4NDkKUldSSnVIQXhuOFdRbmxWZzM3RFFTZWNzcU0vWExFdjRNR3pzbGo4USs4UUFCTkFiUy9MOGpTdzkK";
    const TEST_SIG: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSSnVIQXhuOFdRbm4vZlp4Smxvc0RnUFptSkQ1SzY4ejVGZDRGbDB6aGNoMDk4eGdtWjBibGt4VG1xQ0ZCUGIvOXRvUWRhejVOZkVkZ0xBc3lrUGlVdDFhV3gvcGtmSHcwPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzkxMDc5MzgwCWZpbGU6Zml4dHVyZS5iaW4KZXIyVlNhWXNndng3ZE9aSmhVTDVkSWxSNURkSkJJSjNvRWZ0c0JXQk1jUGh1N3BXTWhQclIzT1d1dmxtenFOc3pyenBxaGMzOS9TQ3U3MGgwb0dKRGc9PQo=";
    const FIXTURE: &[u8] = b"buddy update fixture\n";

    #[test]
    fn tauri_signer_signature_verifies() {
        verify_signature(FIXTURE, TEST_SIG, TEST_KEY).unwrap();
    }

    #[test]
    fn tampered_data_is_rejected() {
        let err = verify_signature(b"buddy update fixture!\n", TEST_SIG, TEST_KEY).unwrap_err();
        assert!(matches!(err, UpdateError::Signature(_)));
    }

    #[test]
    fn signature_from_other_key_is_rejected() {
        // 产品公钥验证测试密钥的签名必须失败：证明公钥确实参与了校验。
        let err = verify_signature(FIXTURE, TEST_SIG, crate::PUBLIC_KEY).unwrap_err();
        assert!(matches!(err, UpdateError::Signature(_)));
    }

    /// 发布私钥 `~/.tauri/buddy-v2.key` 对 b"probe\n" 的真实签名：证明内置公钥与发布私钥配对。
    const PRODUCT_SIG: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUM3AwaHlzQVRCdHAzY1VuZFM4K3lRWjFKQUJ4UTdLWUpzNTdwOEVsZnR5ejgwRGVGcUltS2hmbEJWc2x5TUMzWHEyTk83d1ZnZk93VUlnTDV4TXVmcjNZUy9KMkE3NUFvPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzkxMDgzNTI2CWZpbGU6cHJvYmUyLnR4dAoxcVkvb0dSdTdrZVFUM3loaVd6N0tSY3d2UXZ6NEFyR3IxalFNYnRDK1hFVUFlQ0lFR0EvZ2drZU1FRlBWVUdUZ2hkQWJ2MmRub2RkaVhCbGZNTUJBUT09Cg==";

    #[test]
    fn product_key_verifies_release_signature() {
        verify_signature(b"probe\n", PRODUCT_SIG, crate::PUBLIC_KEY).unwrap();
    }

    #[test]
    fn product_public_key_decodes() {
        assert!(decode_outer(crate::PUBLIC_KEY)
            .and_then(|t| PublicKey::decode(&t).map_err(|e| e.to_string()))
            .is_ok());
    }

    #[test]
    fn garbage_signature_is_rejected() {
        assert!(matches!(
            verify_signature(FIXTURE, "not base64", TEST_KEY),
            Err(UpdateError::Signature(_))
        ));
    }
}
