//! Golden round-trip tests (brief § 8.4, spec § 19 Gate 1).
//!
//! Encode → decode → re-encode is byte-stable, decoding equals the original
//! value, and the committed golden bytes under `fixtures/artifacts/` re-encode
//! to themselves.

use ke_core::canonical::{
    decode_manifest, decode_policy, decode_rule, encode_manifest, encode_policy, encode_rule,
};
use ke_core::examples;
use ke_core::ir::time::JurisdictionDate;
use ke_core::manifest::ArtifactKind;
use std::fs;
use std::path::{Path, PathBuf};

fn artifacts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .join("fixtures")
        .join("artifacts")
}

#[test]
fn rules_encode_decode_reencode_stable() {
    // `encode_rule` canonicalizes (e.g. sorts tag sets), so the encoded bytes
    // are the fixed point: decode → re-encode must be byte-identical. (Decoding
    // does NOT reproduce a non-canonical input verbatim — that is the point.)
    for (id, rule) in examples::rules() {
        let bytes = encode_rule(&rule).expect("encode");
        let decoded = decode_rule(&bytes).expect("decode");
        let reencoded = encode_rule(&decoded).expect("re-encode");
        assert_eq!(bytes, reencoded, "re-encode is byte-stable for {id}");
    }
}

#[test]
fn encoding_is_deterministic() {
    for (id, rule) in examples::rules() {
        let a = encode_rule(&rule).expect("encode a");
        let b = encode_rule(&rule).expect("encode b");
        assert_eq!(a, b, "encode is a pure function for {id}");
    }
}

#[test]
fn policy_round_trips() {
    let (_, bundle) = examples::policy();
    let bytes = encode_policy(&bundle).expect("encode");
    let decoded = decode_policy(&bytes).expect("decode");
    assert_eq!(bytes, encode_policy(&decoded).expect("re-encode"));
}

#[test]
fn manifest_round_trips() {
    let manifest = examples::synthetic_manifest(
        ArtifactKind::RegimePack,
        "mica_2023",
        JurisdictionDate::new(2024, 6, 30),
        b"some-canonical-bytes",
    );
    let bytes = encode_manifest(&manifest).expect("encode");
    let decoded = decode_manifest(&bytes).expect("decode");
    assert_eq!(decoded, manifest);
    assert_eq!(bytes, encode_manifest(&decoded).expect("re-encode"));
}

#[test]
fn golden_rule_files_are_byte_stable() {
    for (id, _) in examples::rules() {
        let path = artifacts_dir().join(&id).join("canonical.bin");
        let bytes = fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "missing golden fixture {} ({e}); run `cargo run -p ke-core --bin gen-fixtures`",
                path.display()
            )
        });
        let decoded = decode_rule(&bytes).expect("golden decode");
        let reencoded = encode_rule(&decoded).expect("golden re-encode");
        assert_eq!(bytes, reencoded, "golden bytes stable for {id}");
    }
}

#[test]
fn golden_policy_file_is_byte_stable() {
    let (id, _) = examples::policy();
    let path = artifacts_dir().join(&id).join("canonical.bin");
    let bytes = fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden fixture {} ({e}); run `cargo run -p ke-core --bin gen-fixtures`",
            path.display()
        )
    });
    let decoded = decode_policy(&bytes).expect("golden decode");
    assert_eq!(bytes, encode_policy(&decoded).expect("golden re-encode"));
}
