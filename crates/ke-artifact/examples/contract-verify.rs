//! The **Rust leg** of the three-language contract test (Gate 4 Phase 4b).
//!
//! Reads a `.kew` and the shared contract inputs, runs the pure 4a
//! [`verify_artifact`] folded call, and prints ONE canonical JSON line:
//!
//! ```json
//! {"verdict":"verified","registry_state":"Published","content_hash":"<hex>","provenance":{...}}
//! ```
//!
//! The Python (wheel) and WASM (node) legs print the identical shape; the
//! contract test diffs the three. `verdict` is `"verified"` or
//! `"rejected:<Debug>"`, and `provenance` is the canonical-JSON object the
//! surface already guarantees is byte-stable — so the comparison is exact.
//!
//! Usage:
//!   cargo run -p ke-artifact --features test-keys --example contract-verify -- \
//!     <artifact.kew> <keydir.json> <context.json> <policy.json> <registry.json> <exported_at_unix>
//!
//! This is verify-only: it loads no signing key and produces no artifact.

use ke_artifact::{verify_artifact, KeyDirectory, PolicyContext, RegistryEvidence, Verdict};
use ke_core::manifest::VerificationPolicy;
use std::fs;
use std::process;

fn read_json<T: serde::de::DeserializeOwned>(label: &str, path: &str) -> T {
    let raw =
        fs::read_to_string(path).unwrap_or_else(|e| fatal(&format!("read {label} {path}: {e}")));
    serde_json::from_str(&raw).unwrap_or_else(|e| fatal(&format!("parse {label} {path}: {e}")))
}

fn fatal(msg: &str) -> ! {
    eprintln!("FATAL: {msg}");
    process::exit(1);
}

fn hex32(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn verdict_str(v: &Verdict) -> String {
    match v {
        Verdict::Verified => "verified".to_string(),
        Verdict::Rejected(reason) => format!("rejected:{reason:?}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 7 {
        fatal(
            "usage: contract-verify <kew> <keydir.json> <context.json> <policy.json> \
             <registry.json> <exported_at_unix>",
        );
    }
    let kew = fs::read(&args[1]).unwrap_or_else(|e| fatal(&format!("read kew {}: {e}", args[1])));
    let keydir: KeyDirectory = read_json("keydir", &args[2]);
    let ctx: PolicyContext = read_json("context", &args[3]);
    let policy: VerificationPolicy = read_json("policy", &args[4]);
    let registry: RegistryEvidence = read_json("registry", &args[5]);
    let exported_at_unix: u64 = args[6]
        .parse()
        .unwrap_or_else(|e| fatal(&format!("parse exported_at_unix {}: {e}", args[6])));

    let outcome = verify_artifact(&kew, &keydir, &ctx, &policy, &registry, exported_at_unix);
    let provenance: serde_json::Value = serde_json::from_str(
        &outcome
            .provenance
            .to_canonical_json()
            .unwrap_or_else(|e| fatal(&format!("provenance json: {e}"))),
    )
    .unwrap_or_else(|e| fatal(&format!("reparse provenance: {e}")));

    let result = serde_json::json!({
        "verdict": verdict_str(&outcome.verdict),
        "registry_state": format!("{:?}", outcome.registry_state),
        "content_hash": hex32(&outcome.provenance.artifact_hash),
        "provenance": provenance,
    });
    // One line, compact — the contract test diffs these verbatim.
    println!("{result}");
}
