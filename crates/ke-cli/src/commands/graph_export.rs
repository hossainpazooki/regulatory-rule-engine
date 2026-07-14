//! `ke graph export`: the ADR-0023 graph exporter — a **consumer under
//! ADR-0019 discipline** over the local registry.
//!
//! Per stored artifact address: read the bytes filed at that address,
//! re-address (decoded `manifest.artifact_hash` must equal the address —
//! mislabeled-but-valid content fails closed), require `Published`, run the
//! folded [`verify_artifact`] under the **kind-selected** policy
//! ([`policy_for_kind`], the ADR-0022 lesson), and only then project nodes +
//! edges ([`extract_graph`] + [`conflict_edges`]). Everything refused lands in
//! the refusal log with its named reason — never as a node.
//!
//! Like `ke serve /verify`, verification needs a verifying-key directory:
//! this build has only the fixed-seed `test-keys` (production key authority is
//! ADR-0009). Without the feature the command reports that honestly.

use crate::commands::export_provenance::read_registry_evidence;
use crate::graph::{conflict_edges, extract_graph, render_cypher, Graph};
use crate::policy::policy_for_kind;
use crate::registry::backend::RegistryBackend;
use crate::registry::hash_hex;
use anyhow::Result;
use ke_artifact::{decode_artifact, ArtifactPayload, RegistryStatus, Verdict};

/// Arguments for `ke graph export`.
pub struct GraphExportArgs<'a> {
    /// Policy environment for the verification context (mirrors serve).
    pub env: &'a str,
    /// Verification time, unix seconds (sourced at the CLI edge).
    pub now_unix: u64,
}

/// One refused artifact: the address it was filed under and the named reason.
/// Refusals are part of the export's honest output — a refused artifact must
/// be visible in the log precisely because it is invisible in the graph.
#[derive(Clone, Debug)]
pub struct Refusal {
    pub artifact_hex: String,
    pub reason: String,
}

/// Outcome of `ke graph export`.
pub struct GraphExportOutcome {
    /// Addresses exported, in address order.
    pub exported: Vec<[u8; 32]>,
    pub refusals: Vec<Refusal>,
    pub graph: Graph,
    /// Idempotent MERGE-only Cypher rendering of `graph`.
    pub cypher: String,
}

/// Run `ke graph export` over every artifact address in the registry.
pub fn run<B: RegistryBackend>(
    backend: &B,
    args: &GraphExportArgs<'_>,
) -> Result<GraphExportOutcome> {
    let mut exported = Vec::new();
    let mut refusals = Vec::new();
    let mut graph = Graph::default();

    for address in backend.list_addresses()? {
        match admit(backend, &address, args)? {
            Admission::Refused(reason) => refusals.push(Refusal {
                artifact_hex: hash_hex(&address),
                reason,
            }),
            Admission::Verified(artifact) => {
                let hex = hash_hex(&address);
                let mut projection = extract_graph(&artifact);
                if let ArtifactPayload::Rules(rules) = &artifact.payload {
                    projection.edges.extend(conflict_edges(&hex, rules));
                }
                graph.merge(projection);
                exported.push(address);
            }
        }
    }

    let cypher = render_cypher(&graph);
    Ok(GraphExportOutcome {
        exported,
        refusals,
        graph,
        cypher,
    })
}

/// Persist an export: `<out>/graph.cypher` (the idempotent MERGE script) and
/// `<out>/refusals.log` (one `<hex><TAB><reason>` line per refusal — the
/// honest record of what is NOT in the graph and why).
pub fn write_outputs(outcome: &GraphExportOutcome, out_dir: &str) -> Result<()> {
    let dir = std::path::Path::new(out_dir);
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("graph.cypher"), &outcome.cypher)?;
    let mut log = String::new();
    for r in &outcome.refusals {
        use std::fmt::Write as _;
        let _ = writeln!(log, "{}\t{}", r.artifact_hex, r.reason);
    }
    std::fs::write(dir.join("refusals.log"), log)?;
    Ok(())
}

/// One artifact's admission decision: verified and projectable, or refused
/// with a named reason.
enum Admission {
    Verified(Box<ke_artifact::Artifact>),
    Refused(String),
}

/// Admit or refuse the artifact filed at `address` (ADR-0023 D1, in order):
/// readable → decodable → correctly addressed → `Published` → folded verify
/// under the kind-selected policy. First failure names the refusal.
fn admit<B: RegistryBackend>(
    backend: &B,
    address: &[u8; 32],
    args: &GraphExportArgs<'_>,
) -> Result<Admission> {
    let kew = match backend.read_artifact_kew(address) {
        Ok(kew) => kew,
        Err(e) => return Ok(Admission::Refused(format!("read failed: {e}"))),
    };
    let (artifact, _) = match decode_artifact(&kew) {
        Ok(decoded) => decoded,
        Err(e) => return Ok(Admission::Refused(format!("decode failed: {e}"))),
    };

    // Re-address: the decoded manifest hash must equal the address the bytes
    // were filed under. A fully-valid artifact planted at another address
    // passes folded verify — this check is what refuses it.
    if artifact.manifest.artifact_hash != *address {
        return Ok(Admission::Refused(format!(
            "address mismatch: stored bytes claim {}, filed under {}",
            hash_hex(&artifact.manifest.artifact_hash),
            hash_hex(address)
        )));
    }

    // Registry gate: Published only; everything else is blocked even with
    // valid crypto, and `Unknown` fails closed (ADR-0019 consumer rule).
    let evidence = read_registry_evidence(backend, address)?;
    if evidence.status != RegistryStatus::Published {
        return Ok(Admission::Refused(format!(
            "not published: registry status {:?}",
            evidence.status
        )));
    }

    let outcome = verify_outcome(&kew, &artifact, args, evidence)?;
    match outcome.verdict {
        Verdict::Verified => Ok(Admission::Verified(Box::new(artifact))),
        Verdict::Rejected(reason) => Ok(Admission::Refused(format!("verify rejected: {reason:?}"))),
    }
}

/// Folded verify with the fixed-seed test-key directory (mirrors
/// `serve::handlers::run_verify` — see the refactor note there).
#[cfg(any(test, feature = "test-keys"))]
fn verify_outcome(
    kew: &[u8],
    artifact: &ke_artifact::Artifact,
    args: &GraphExportArgs<'_>,
    evidence: ke_artifact::RegistryEvidence,
) -> Result<ke_artifact::VerificationOutcome> {
    use ke_artifact::{verify_artifact, PolicyContext};

    let mut supported: Vec<String> = artifact
        .attestations
        .iter()
        .map(|a| a.attestation_policy_version.clone())
        .collect();
    supported.sort();
    supported.dedup();

    let ctx = PolicyContext {
        environment: args.env.to_string(),
        now_unix: args.now_unix,
        supported_policy_versions: supported,
        current_legal_source_hash: None,
    };
    let keydir = crate::serve::handlers::test_key_directory(artifact);
    let policy = policy_for_kind(artifact.manifest.artifact_kind);

    Ok(verify_artifact(
        kew,
        &keydir,
        &ctx,
        &policy,
        &evidence,
        args.now_unix,
    ))
}

#[cfg(not(any(test, feature = "test-keys")))]
fn verify_outcome(
    _kew: &[u8],
    _artifact: &ke_artifact::Artifact,
    _args: &GraphExportArgs<'_>,
    _evidence: ke_artifact::RegistryEvidence,
) -> Result<ke_artifact::VerificationOutcome> {
    anyhow::bail!(
        "`ke graph export` needs a compiler/expert verifying-key directory; this build has \
         none. Production key authority is an open decision (spec § 21, Gate 4). Build with \
         `--features test-keys` to verify against the fixed-seed test keys."
    )
}
