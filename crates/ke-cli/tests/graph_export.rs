//! ADR-0023 graph export: integration tests.
//!
//! Drives real artifacts through the existing command `run` helpers (compile →
//! ml-check → attest → publish) in a tempdir registry with fixed keys + a fixed
//! clock, then exercises the graph extraction / export surface against them.
//! No mocks: every artifact is a genuine `.kew` from the real compile path.
//!
//! Determinism mirrors `export_provenance.rs`: fixed `NOW`, tempdir backend,
//! fixed-seed test keys (feature unification gives this target the gated
//! modules).

use ke_cli::commands::{attest, compile, graph_export, ml_check, publish};
use ke_cli::graph::{
    blast_radius, conflict_edges, conflict_exposure, extract_graph, EdgeLabel, NodeLabel,
};
use ke_cli::registry::backend::{LocalFsBackend, RegistryBackend};
use ke_cli::registry::{hash_hex, LifecycleState};
use ke_core::manifest::AttestationType;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const NOW: u64 = 1_750_000_000;
const FIXTURE_YAML: &str = "../../fixtures/rules/mica_stablecoin.yaml";
const FIXTURE_REGIME: &str = "mica_2023";

const FULL_SET: [AttestationType; 3] = [
    AttestationType::SourceFidelity,
    AttestationType::ScenarioCoverage,
    AttestationType::PublicationApproval,
];

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("ke-graph-test-{label}-{pid}-{n}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create tempdir");
        TempDir { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn fixture_path(rel: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

fn compile_into(tmp: &TempDir, yaml_rel: &str, regime: &str) -> (LocalFsBackend, [u8; 32]) {
    let backend = LocalFsBackend::open(&tmp.path).expect("open backend");
    let yaml = fixture_path(yaml_rel);
    let outcome = compile::run(
        &backend,
        &compile::CompileArgs {
            yaml_path: &yaml,
            regime_id: regime,
            env: "local",
            now_unix: NOW,
        },
    )
    .expect("compile run");
    assert_eq!(outcome.final_state, LifecycleState::StructurallyVerified);
    (backend, outcome.artifact_hash)
}

/// Drive an already-compiled artifact to `Published`.
fn drive_to_published(backend: &LocalFsBackend, hash: [u8; 32]) {
    ml_check::run(
        backend,
        &ml_check::MlCheckArgs {
            artifact_hash: hash,
            now_unix: NOW,
        },
    )
    .expect("ml-check");
    attest::run(
        backend,
        &attest::AttestArgs {
            artifact_hash: hash,
            types: &FULL_SET,
            now_unix: NOW,
        },
    )
    .expect("attest");
    publish::run(
        backend,
        &publish::PublishArgs {
            artifact_hash: hash,
            env: "staging",
            tag: "current",
            policy_path: None,
            now_unix: NOW,
        },
    )
    .expect("publish");
}

fn decode(backend: &LocalFsBackend, hash: &[u8; 32]) -> ke_artifact::Artifact {
    let kew = backend.read_artifact_kew(hash).expect("read kew");
    let (artifact, _) = ke_artifact::decode_artifact(&kew).expect("decode");
    artifact
}

// ---------------------------------------------------------------------------
// graph export command: a consumer under ADR-0019 discipline (ADR-0023 D1).
// Verified + Published only; everything else is a refusal in the log, never a
// node. No mocks — every scenario is a real registry in a tempdir.
// ---------------------------------------------------------------------------

fn run_export(backend: &LocalFsBackend) -> graph_export::GraphExportOutcome {
    graph_export::run(
        backend,
        &graph_export::GraphExportArgs {
            // Mock-TSA-stamped attestations verify only under the `local`
            // policy environment (R8) — same default `ke serve /verify` uses.
            env: "local",
            now_unix: NOW,
        },
    )
    .expect("graph export run")
}

#[test]
fn export_includes_published_and_refuses_unpublished() {
    let tmp = TempDir::new("export-gate");
    // Two real artifacts in one registry: one driven to Published, one left
    // at StructurallyVerified.
    let (backend, published) = compile_into(&tmp, FIXTURE_YAML, FIXTURE_REGIME);
    drive_to_published(&backend, published);
    let unpublished = {
        let yaml = fixture_path("../../fixtures/rules/fca_crypto.yaml");
        let outcome = compile::run(
            &backend,
            &compile::CompileArgs {
                yaml_path: &yaml,
                regime_id: "fca_crypto_2024",
                env: "local",
                now_unix: NOW,
            },
        )
        .expect("compile second");
        outcome.artifact_hash
    };

    let outcome = run_export(&backend);

    // Vacuity guard + gate: exactly the published artifact exports.
    assert_eq!(
        outcome.exported,
        vec![published],
        "refusals: {:?}",
        outcome.refusals
    );

    // The unpublished artifact is a refusal with its status named — never a node.
    let unpub_hex = hash_hex(&unpublished);
    let refusal = outcome
        .refusals
        .iter()
        .find(|r| r.artifact_hex == unpub_hex)
        .expect("unpublished artifact is refused");
    assert!(
        refusal.reason.contains("not published"),
        "refusal names the gate: {}",
        refusal.reason
    );
    assert!(
        !outcome.cypher.contains(&unpub_hex),
        "refused artifact leaves no trace in the export"
    );
    // The published artifact's node is present in the rendered Cypher.
    assert!(outcome.cypher.contains(&hash_hex(&published)));
    assert!(outcome.cypher.contains("MERGE"));
}

#[test]
fn export_refuses_valid_artifact_at_wrong_address() {
    // A fully-valid artifact planted at another artifact's address passes
    // folded verify (its content is internally consistent, and the registry
    // evidence at the address says Published) — ONLY the exporter's
    // address ≡ manifest-hash check catches it. This is what makes the
    // re-address check non-vacuous (ADR-0023 D1).
    let tmp_a = TempDir::new("addr-a");
    let (backend, hash_a) = compile_into(&tmp_a, FIXTURE_YAML, FIXTURE_REGIME);
    drive_to_published(&backend, hash_a);

    // Source a different, internally-valid kew from a second registry.
    let tmp_b = TempDir::new("addr-b");
    let (backend_b, hash_b) = compile_into(
        &tmp_b,
        "../../fixtures/rules/fca_crypto.yaml",
        "fca_crypto_2024",
    );
    let kew_b = backend_b.read_artifact_kew(&hash_b).expect("read b");
    assert_ne!(hash_a, hash_b);

    // Plant B's bytes at A's address.
    let a_dir = tmp_a.path.join("artifacts").join(hash_hex(&hash_a));
    std::fs::write(a_dir.join("artifact.kew"), &kew_b).expect("plant");

    let outcome = run_export(&backend);

    assert!(outcome.exported.is_empty(), "nothing exports");
    assert!(
        outcome
            .refusals
            .iter()
            .any(|r| r.reason.contains("address")),
        "the address mismatch is the named refusal: {:?}",
        outcome.refusals
    );
}

#[test]
fn write_outputs_persists_cypher_and_refusal_log() {
    let tmp = TempDir::new("write-outputs");
    let (backend, published) = compile_into(&tmp, FIXTURE_YAML, FIXTURE_REGIME);
    drive_to_published(&backend, published);
    // One extra unpublished artifact so the refusal log is non-empty.
    let yaml = fixture_path("../../fixtures/rules/fca_crypto.yaml");
    compile::run(
        &backend,
        &compile::CompileArgs {
            yaml_path: &yaml,
            regime_id: "fca_crypto_2024",
            env: "local",
            now_unix: NOW,
        },
    )
    .expect("compile second");

    let outcome = run_export(&backend);
    let out_dir = tmp.path.join("graph-out");
    graph_export::write_outputs(&outcome, out_dir.to_str().unwrap()).expect("write outputs");

    let cypher = std::fs::read_to_string(out_dir.join("graph.cypher")).expect("cypher written");
    assert_eq!(cypher, outcome.cypher);
    assert!(cypher.contains("MERGE"), "vacuity guard");

    let refusals = std::fs::read_to_string(out_dir.join("refusals.log")).expect("log written");
    assert_eq!(
        refusals.lines().count(),
        outcome.refusals.len(),
        "one line per refusal"
    );
    assert!(
        refusals.contains("not published"),
        "refusal reasons are in the log: {refusals}"
    );
}

#[test]
fn export_carries_conflict_edges_for_a_published_conflicted_pack() {
    let tmp = TempDir::new("export-conflicts");
    // The overlapping-scope pack has a ReviewRequired (non-blocking) conflict,
    // so it genuinely publishes — and its conflict edge must ride the export.
    let (backend, hash) = compile_into(
        &tmp,
        "../ke-compiler/tests/fixtures/conflicts/overlapping_scope.yaml",
        "conflict_demo",
    );
    drive_to_published(&backend, hash);

    let outcome = run_export(&backend);
    assert_eq!(outcome.exported, vec![hash]);
    assert!(
        outcome.cypher.contains("CONFLICTS_WITH"),
        "recomputed conflict edge rides the export"
    );
}

// ---------------------------------------------------------------------------
// Oracles (ADR-0023 D6): the Rust side of each Cypher differential. Semantics
// are chosen to be EXACTLY decidable on both sides: pure reachability over
// the bipartite citation graph (BFS ≡ Cypher var-length trail reachability),
// then a single conflict hop — never a path predicate, whose relationship-
// reuse rules would let the two sides legally disagree.
// ---------------------------------------------------------------------------

/// Publish the two harness-corpus packs into one registry and export.
fn harness_graph(tmp: &TempDir) -> (ke_cli::graph::Graph, String, String) {
    let (backend, hash_a) = compile_into(tmp, "tests/fixtures/graph/regime_a.yaml", "regime_a");
    drive_to_published(&backend, hash_a);
    let yaml_b = fixture_path("tests/fixtures/graph/regime_b.yaml");
    let hash_b = compile::run(
        &backend,
        &compile::CompileArgs {
            yaml_path: &yaml_b,
            regime_id: "regime_b",
            env: "local",
            now_unix: NOW,
        },
    )
    .expect("compile regime_b")
    .artifact_hash;
    drive_to_published(&backend, hash_b);

    let outcome = run_export(&backend);
    assert_eq!(
        outcome.exported.len(),
        2,
        "refusals: {:?}",
        outcome.refusals
    );
    (outcome.graph, hash_hex(&hash_a), hash_hex(&hash_b))
}

#[test]
fn blast_radius_walks_citations_to_artifacts_and_attestations() {
    let tmp = TempDir::new("oracle-blast");
    let (graph, hex_a, hex_b) = harness_graph(&tmp);

    // Amending doc_shared reaches: both citing rules, their artifacts, and
    // every attestation downstream of those artifacts — and nothing of
    // doc_b_only's exclusive neighborhood beyond the shared artifact.
    let radius = blast_radius(&graph, "doc_shared", None);

    let mut expected = vec![
        format!("{hex_a}:alpha_licensing"),
        format!("{hex_b}:beta_one"),
        hex_a.clone(),
        hex_b.clone(),
    ];
    // FULL_SET attests three types per artifact.
    for hex in [&hex_a, &hex_b] {
        for i in 0..3 {
            expected.push(format!("{hex}:att:{i}"));
        }
    }
    expected.sort();

    assert!(!radius.is_empty(), "vacuity guard");
    assert_eq!(radius, expected);
    // beta_two is NOT in the radius: it cites only doc_b_only.
    assert!(!radius.contains(&format!("{hex_b}:beta_two")));
}

#[test]
fn blast_radius_narrows_by_article() {
    let tmp = TempDir::new("oracle-blast-article");
    let (graph, hex_a, hex_b) = harness_graph(&tmp);

    // Article 12 of doc_shared is cited by both rules; a different article
    // reaches nothing.
    let hit = blast_radius(&graph, "doc_shared", Some("12"));
    assert!(hit.contains(&format!("{hex_a}:alpha_licensing")));
    assert!(hit.contains(&format!("{hex_b}:beta_one")));
    assert!(blast_radius(&graph, "doc_shared", Some("99")).is_empty());
}

#[test]
fn conflict_exposure_crosses_regimes_via_shared_citation_then_conflict() {
    let tmp = TempDir::new("oracle-exposure");
    let (graph, _hex_a, hex_b) = harness_graph(&tmp);

    // regime_a's alpha_licensing shares doc_shared with beta_one; beta_one
    // conflicts (overlapping scope) with beta_two. Exposure surfaces the
    // conflict-hop endpoint in the target regime.
    let exposure = conflict_exposure(&graph, "regime_a", "regime_b");
    assert_eq!(exposure, vec![format!("{hex_b}:beta_two")]);

    // No conflicts are reachable in the reverse direction into regime_a
    // (regime_a's pack is conflict-free).
    assert!(conflict_exposure(&graph, "regime_b", "regime_a").is_empty());
}

// ---------------------------------------------------------------------------
// conflict_edges: CONFLICTS_WITH by deterministic recompute (ADR-0023 D4).
// The recompute calls the same ke-compiler verify() the publish gate ran, over
// the decoded signed payload — recompute ≡ what the gate saw.
// ---------------------------------------------------------------------------

#[test]
fn conflict_edges_recompute_matches_the_gate_and_is_deterministic() {
    // A publishable (non-blocking) conflicting pair: the ke-compiler T4
    // fixture with an overlapping-scope, review-required conflict.
    let src = std::fs::read_to_string(fixture_path(
        "../ke-compiler/tests/fixtures/conflicts/overlapping_scope.yaml",
    ))
    .expect("read conflict fixture");
    let rules = ke_compiler::compile_rules(&src).expect("compile fixture");

    let hex = "deadbeef"; // edge ids are scoped by the caller's artifact hex
    let edges = conflict_edges(hex, &rules);

    // Vacuity guard: the fixture is known-conflicting; empty edges = red.
    assert!(!edges.is_empty(), "known-conflicting fixture yields edges");

    // Every edge mirrors a conflict the gate's verify() reports.
    let report = ke_compiler::verify::verify(&rules);
    assert_eq!(edges.len(), report.conflicts.len());

    // The overlapping-scope pair is present, endpoints in sorted order (the
    // conflict is symmetric; one deterministic edge, not two).
    let edge = edges
        .iter()
        .find(|e| {
            e.label == EdgeLabel::ConflictsWith
                && e.from == format!("{hex}:scope_a")
                && e.to == format!("{hex}:scope_b")
        })
        .expect("scope_a -> scope_b conflict edge, endpoints sorted");
    assert!(edge
        .props
        .iter()
        .any(|(k, v)| *k == "class" && v == "OverlappingScope"));
    assert!(edge
        .props
        .iter()
        .any(|(k, v)| *k == "severity" && v == "ReviewRequired"));

    // Deterministic: recompute twice, byte-identical edge list.
    assert_eq!(edges, conflict_edges(hex, &rules));
}

// ---------------------------------------------------------------------------
// Golden-edge fixture pin (ADR-0023 D4). The committed fixture
// `fixtures/graph/expected_edges.json` is generated by `gen-graph-fixture`
// (never hand-edited, per the fixtures/ rule) and pins the FULL edge set —
// every label, not just CONFLICTS_WITH — recomputed over every golden. The
// full set keeps the pin inherently non-vacuous (goldens are small clean
// packs whose conflict set may legitimately be empty; CITES/SPANS/ATTESTED_BY
// are not). Drift in the conflict gate, the extraction, or the goldens turns
// this red instead of silently reshaping the graph. Non-vacuity of the
// conflict machinery itself is proven by
// `conflict_edges_recompute_matches_the_gate_and_is_deterministic`.
// ---------------------------------------------------------------------------

#[test]
fn golden_edges_match_the_committed_fixture() {
    let computed = ke_cli::graph::golden_edges_json(&fixture_path("../../fixtures"))
        .expect("recompute golden edges");

    let fixture_raw = std::fs::read_to_string(fixture_path(
        "../../fixtures/graph/expected_edges.json",
    ))
    .expect("read committed fixture (generate with `cargo run -p ke-cli --bin gen-graph-fixture`)");
    let expected: serde_json::Value = serde_json::from_str(&fixture_raw).expect("fixture parses");

    // Vacuity guard: the pin must actually pin something — every golden
    // present, and a non-empty edge set overall.
    let goldens = expected["goldens"].as_object().expect("goldens object");
    assert!(goldens.len() >= 3, "all goldens pinned: {}", goldens.len());
    assert!(
        goldens
            .values()
            .any(|g| !g["edges"].as_array().expect("edges array").is_empty()),
        "vacuity guard: at least one golden has edges"
    );

    assert_eq!(
        computed, expected,
        "recomputed golden edges differ from the committed fixture — if the \
         change is intentional, regenerate via `cargo run -p ke-cli --bin \
         gen-graph-fixture` and review the diff; never hand-edit"
    );
}

// ---------------------------------------------------------------------------
// extract_graph: pure projection of one decoded artifact into nodes + edges
// ---------------------------------------------------------------------------

#[test]
fn extract_graph_projects_rules_artifact_nodes_and_edges() {
    let tmp = TempDir::new("extract-rules");
    let (backend, hash) = compile_into(&tmp, FIXTURE_YAML, FIXTURE_REGIME);
    let artifact = decode(&backend, &hash);
    let hex = hash_hex(&hash);

    let rules = match &artifact.payload {
        ke_artifact::ArtifactPayload::Rules(rules) => rules.clone(),
        other => panic!("fixture compiles to a Rules payload, got {other:?}"),
    };
    assert!(!rules.is_empty(), "vacuity guard: fixture pack has rules");

    let graph = extract_graph(&artifact);

    // One Artifact node, id = content-hash hex, carrying the kind.
    let artifact_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.label == NodeLabel::Artifact)
        .collect();
    assert_eq!(artifact_nodes.len(), 1);
    assert_eq!(artifact_nodes[0].id, hex);
    assert!(
        artifact_nodes[0]
            .props
            .iter()
            .any(|(k, v)| *k == "kind" && v == "RegimePack"),
        "artifact node carries its kind: {:?}",
        artifact_nodes[0].props
    );

    // One Regime node with an IN_REGIME edge from the artifact.
    assert!(graph
        .nodes
        .iter()
        .any(|n| n.label == NodeLabel::Regime && n.id == FIXTURE_REGIME));
    assert!(graph
        .edges
        .iter()
        .any(|e| e.label == EdgeLabel::InRegime && e.from == hex && e.to == FIXTURE_REGIME));

    // One Rule node per payload rule, each contained by the artifact and each
    // citing its source document.
    for rule in &rules {
        let rule_node_id = format!("{hex}:{}", rule.rule_id);
        assert!(
            graph
                .nodes
                .iter()
                .any(|n| n.label == NodeLabel::Rule && n.id == rule_node_id),
            "rule node {rule_node_id} present"
        );
        assert!(
            graph
                .edges
                .iter()
                .any(|e| e.label == EdgeLabel::Contains && e.from == hex && e.to == rule_node_id),
            "CONTAINS {hex} -> {rule_node_id}"
        );
        let cites = graph
            .edges
            .iter()
            .find(|e| {
                e.label == EdgeLabel::Cites
                    && e.from == rule_node_id
                    && e.to == rule.source.document_id
            })
            .unwrap_or_else(|| panic!("CITES {rule_node_id} -> {}", rule.source.document_id));
        // Article-level granularity is recorded on the rule's DocumentRef and
        // must survive onto the edge (the blast-radius query keys off it).
        if let Some(article) = &rule.source.article {
            assert!(
                cites
                    .props
                    .iter()
                    .any(|(k, v)| *k == "article" && v == article),
                "CITES carries article {article}: {:?}",
                cites.props
            );
        }
        // The cited document is a CorpusDoc node.
        assert!(graph
            .nodes
            .iter()
            .any(|n| n.label == NodeLabel::CorpusDoc && n.id == rule.source.document_id));
    }
    let rule_node_count = graph
        .nodes
        .iter()
        .filter(|n| n.label == NodeLabel::Rule)
        .count();
    assert_eq!(rule_node_count, rules.len(), "exactly one node per rule");

    // Vacuity guard for the citation substrate: at least one rule in the
    // fixture records an article, so at least one CITES edge carries one.
    assert!(
        rules.iter().any(|r| r.source.article.is_some()),
        "fixture records article-level citations"
    );

    // SPANS edges mirror the span index exactly. The current rule corpus
    // records no node-level spans (rule-side granularity is the DocumentRef),
    // so the mirror may legitimately be empty here — the IntentSpec test
    // covers the non-empty SPANS arm from payload source_spans.
    let expected_spans: usize = artifact
        .source_span_index
        .entries
        .iter()
        .map(|e| e.spans.len())
        .sum();
    let span_edges = graph
        .edges
        .iter()
        .filter(|e| e.label == EdgeLabel::Spans)
        .count();
    assert_eq!(
        span_edges, expected_spans,
        "one SPANS edge per indexed span"
    );

    // Compile-time artifact carries no attestations yet — no attestation nodes.
    assert!(artifact.attestations.is_empty());
    assert!(!graph
        .nodes
        .iter()
        .any(|n| n.label == NodeLabel::Attestation));
}

#[test]
fn extract_graph_projects_intentspec_golden_spans_and_attestations() {
    // The committed IntentSpec golden is the real, folded-verify-passing
    // artifact (post-ADR-0022); read-only use of fixtures/.
    let kew = std::fs::read(fixture_path(
        "../../fixtures/artifacts/intentspec_payment/artifact.kew",
    ))
    .expect("read intentspec golden");
    let (artifact, _) = ke_artifact::decode_artifact(&kew).expect("decode golden");
    let hex = hash_hex(&artifact.manifest.artifact_hash);

    let spec = match &artifact.payload {
        ke_artifact::ArtifactPayload::IntentSpec(spec) => spec.clone(),
        other => panic!("golden is an IntentSpec payload, got {other:?}"),
    };
    assert!(
        !spec.source_spans.is_empty(),
        "vacuity guard: golden carries payload source spans"
    );
    assert!(
        !artifact.attestations.is_empty(),
        "vacuity guard: golden carries attestations"
    );

    let graph = extract_graph(&artifact);

    // Artifact node discriminates the kind.
    let artifact_node = graph
        .nodes
        .iter()
        .find(|n| n.label == NodeLabel::Artifact && n.id == hex)
        .expect("artifact node");
    assert!(artifact_node
        .props
        .iter()
        .any(|(k, v)| *k == "kind" && v == "IntentSpec"));

    // One IntentSpec node carrying the action class, contained by the artifact.
    let intent_id = format!("{hex}:intent:{}", spec.action_class);
    assert!(graph
        .nodes
        .iter()
        .any(|n| n.label == NodeLabel::IntentSpec && n.id == intent_id));
    assert!(graph
        .edges
        .iter()
        .any(|e| e.label == EdgeLabel::Contains && e.from == hex && e.to == intent_id));

    // One SPANS edge per payload source span, IntentSpec -> spanned document,
    // article granularity riding the edge when recorded.
    let span_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| e.label == EdgeLabel::Spans && e.from == intent_id)
        .collect();
    assert_eq!(span_edges.len(), spec.source_spans.len());
    for span in &spec.source_spans {
        assert!(
            span_edges.iter().any(|e| e.to == span.document_id
                && span
                    .article
                    .as_ref()
                    .is_none_or(|a| e.props.iter().any(|(k, v)| *k == "article" && v == a))),
            "SPANS -> {} with article carried",
            span.document_id
        );
        assert!(graph
            .nodes
            .iter()
            .any(|n| n.label == NodeLabel::CorpusDoc && n.id == span.document_id));
    }

    // One Attestation node per attestation, ATTESTED_BY from the artifact,
    // carrying type + signer.
    let att_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.label == NodeLabel::Attestation)
        .collect();
    assert_eq!(att_nodes.len(), artifact.attestations.len());
    for (i, att) in artifact.attestations.iter().enumerate() {
        let att_id = format!("{hex}:att:{i}");
        let node = att_nodes
            .iter()
            .find(|n| n.id == att_id)
            .unwrap_or_else(|| panic!("attestation node {att_id}"));
        assert!(node
            .props
            .iter()
            .any(|(k, v)| *k == "type" && *v == format!("{:?}", att.attestation_type)));
        assert!(node
            .props
            .iter()
            .any(|(k, v)| *k == "signer_key_id" && *v == att.key_id));
        assert!(graph
            .edges
            .iter()
            .any(|e| e.label == EdgeLabel::AttestedBy && e.from == hex && e.to == att_id));
    }
}
