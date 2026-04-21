//! 跨 crate 回归：打包侧与校验侧产出的 canonical payload 字节必须完全一致。
//!
//! 背景：`build-tools::sign_manifest` 用 `api_contracts::IntegrityManifest::canonical_payload_bytes()`
//! 签名，而运行时校验走 `security_core::integrity::canonicalize_manifest(&ManifestPayload)`。
//! 两侧若字段顺序 / 类型 / 序列化规则偏差一个字节，Ed25519 校验就会直接失败，
//! 历史签过的包也会集体不可验证。用本测试锁住字节串一致性。

use api_contracts::{IntegrityManifest, IntegrityManifestFile};
use security_core::integrity::{canonicalize_manifest, Mf, ManifestPayload};

fn fixture_files() -> (Vec<IntegrityManifestFile>, Vec<Mf>) {
    let api = vec![
        IntegrityManifestFile {
            path: "bin/app".into(),
            sha256: "a".repeat(64),
        },
        IntegrityManifestFile {
            path: "apps/ui/dist/index.html".into(),
            sha256: "b".repeat(64),
        },
    ];
    let sec = api
        .iter()
        .map(|f| Mf {
            path: f.path.clone(),
            sha256: f.sha256.clone(),
        })
        .collect();
    (api, sec)
}

#[test]
fn canonical_bytes_match_between_build_tools_and_security_core() {
    let (api_files, sec_files) = fixture_files();

    let api_manifest = IntegrityManifest {
        version: 1,
        generated_at: "2026-04-20T00:00:00Z".into(),
        files: api_files,
        // signature 不参与 canonical 字节串，随意值
        signature: "ignored-by-canonical".into(),
    };

    let sec_payload = ManifestPayload {
        version: api_manifest.version,
        generated_at: api_manifest.generated_at.clone(),
        files: sec_files,
    };

    let bytes_from_api = api_manifest
        .canonical_payload_bytes()
        .expect("api_contracts canonical serialization should not fail");
    let bytes_from_sec = canonicalize_manifest(&sec_payload)
        .expect("security_core canonicalize_manifest should not fail");

    assert_eq!(
        bytes_from_api, bytes_from_sec,
        "跨 crate canonical 字节串必须完全一致；若不等则现有已签名包将立即无法验签"
    );
}

#[test]
fn canonical_bytes_snapshot_preserves_field_order_and_types() {
    // 锁定 canonical JSON 的具体字节串，一旦字段顺序或类型序列化发生变化立即失败。
    let manifest = IntegrityManifest {
        version: 1,
        generated_at: "2026-04-20T00:00:00Z".into(),
        files: vec![IntegrityManifestFile {
            path: "bin/app".into(),
            sha256: "ff".repeat(32),
        }],
        signature: "snapshot-signature".into(),
    };
    let bytes = manifest.canonical_payload_bytes().unwrap();
    let as_text = String::from_utf8(bytes).expect("canonical JSON should be valid utf-8");
    let expected = r#"{"version":1,"generated_at":"2026-04-20T00:00:00Z","files":[{"path":"bin/app","sha256":"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"}]}"#;
    assert_eq!(as_text, expected);
}
