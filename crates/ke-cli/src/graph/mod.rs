//! ADR-0023 graph export: a **derived, read-only** property-graph projection of
//! verified artifacts. Never an input to a decision — the deterministic
//! tree-walk engine over signed artifacts remains the only decision path.
//!
//! The node/edge vocabulary is the recorded substrate only (ADR-0023 as
//! amended at build time): no `Jurisdiction` node (the manifest records
//! `regime_id`, so the honest grouping is a `Regime` node), no `SUPERSEDES`
//! edge (the registry records no structured lineage), no `Premise` node
//! (conditions live inside `decision_tree`; the recorded citation substrate is
//! spans + documents).

use ke_artifact::{Artifact, ArtifactPayload};

use crate::registry::hash_hex;

// `graph_export` (the command) reaches the serve-owned test key directory via
// `crate::serve`; nothing in this module touches keys — extraction stays pure.

/// A property-graph node. `id` is globally unique across the export and stable
/// across runs (pure function of the signed content).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub label: NodeLabel,
    pub id: String,
    pub props: Vec<(&'static str, String)>,
}

/// A directed edge between two node ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    pub label: EdgeLabel,
    pub from: String,
    pub to: String,
    pub props: Vec<(&'static str, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeLabel {
    Artifact,
    Regime,
    Rule,
    CorpusDoc,
    Attestation,
    IntentSpec,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeLabel {
    /// Artifact → Regime (from `manifest.regime_id`).
    InRegime,
    /// Artifact → Rule / Artifact → IntentSpec (payload membership).
    Contains,
    /// Rule → CorpusDoc (from `RuleIR.source.document_id`).
    Cites,
    /// Rule → CorpusDoc / IntentSpec → CorpusDoc (from the span index /
    /// payload `source_spans`; span detail rides as edge properties).
    Spans,
    /// Artifact → Attestation (from the appended, kind-selected attestation
    /// set — ADR-0022).
    AttestedBy,
    /// Rule ↔ Rule (symmetric; emitted once with endpoints sorted).
    /// **Deterministic recompute** of the T4 report the publish gate ran —
    /// the report is not persisted anywhere, and `verify()` is a pure
    /// function of the signed payload, so recompute ≡ recorded (ADR-0023 D4).
    ConflictsWith,
}

/// The projected graph of one or more artifacts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Graph {
    /// Merge another artifact's projection in. Shared nodes (`Regime`,
    /// `CorpusDoc` reached by several artifacts) dedupe by `(label, id)`;
    /// edges dedupe exactly.
    pub fn merge(&mut self, other: Graph) {
        for node in other.nodes {
            if !self
                .nodes
                .iter()
                .any(|n| n.label == node.label && n.id == node.id)
            {
                self.nodes.push(node);
            }
        }
        for edge in other.edges {
            if !self.edges.contains(&edge) {
                self.edges.push(edge);
            }
        }
    }
}

/// Escape a string for a single-quoted Cypher literal.
fn cypher_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// Render the graph as idempotent Cypher (`MERGE`-only, so re-loading the
/// same export is a no-op — the read-side mirror of the exporter's
/// determinism). One statement per line; loadable via `cypher-shell`.
pub fn render_cypher(graph: &Graph) -> String {
    let mut out = String::new();
    for node in &graph.nodes {
        out.push_str(&format!(
            "MERGE (n:{} {{id: {}}})",
            node.label.name(),
            cypher_str(&node.id)
        ));
        for (k, v) in &node.props {
            out.push_str(&format!(" SET n.{k} = {}", cypher_str(v)));
        }
        out.push_str(";\n");
    }
    for edge in &graph.edges {
        out.push_str(&format!(
            "MATCH (a {{id: {}}}), (b {{id: {}}}) MERGE (a)-[r:{}]->(b)",
            cypher_str(&edge.from),
            cypher_str(&edge.to),
            edge.label.name()
        ));
        for (k, v) in &edge.props {
            out.push_str(&format!(" SET r.{k} = {}", cypher_str(v)));
        }
        out.push_str(";\n");
    }
    out
}

impl NodeLabel {
    /// The Neo4j label name.
    pub fn name(&self) -> &'static str {
        match self {
            NodeLabel::Artifact => "Artifact",
            NodeLabel::Regime => "Regime",
            NodeLabel::Rule => "Rule",
            NodeLabel::CorpusDoc => "CorpusDoc",
            NodeLabel::Attestation => "Attestation",
            NodeLabel::IntentSpec => "IntentSpec",
        }
    }
}

impl EdgeLabel {
    /// The Neo4j relationship-type name.
    pub fn name(&self) -> &'static str {
        match self {
            EdgeLabel::InRegime => "IN_REGIME",
            EdgeLabel::Contains => "CONTAINS",
            EdgeLabel::Cites => "CITES",
            EdgeLabel::Spans => "SPANS",
            EdgeLabel::AttestedBy => "ATTESTED_BY",
            EdgeLabel::ConflictsWith => "CONFLICTS_WITH",
        }
    }
}

/// Node id for a rule: scoped by artifact hash — the same `rule_id` in two
/// artifacts is two nodes (T4 conflicts are intra-artifact).
fn rule_node_id(artifact_hex: &str, rule_id: &str) -> String {
    format!("{artifact_hex}:{rule_id}")
}

/// Project one decoded artifact into nodes + edges. **Pure** — no I/O, no RNG;
/// callers gate on verification before projecting (ADR-0023 Decision 1).
pub fn extract_graph(artifact: &Artifact) -> Graph {
    let hex = hash_hex(&artifact.manifest.artifact_hash);
    let regime = artifact.manifest.regime_id.clone();

    let mut graph = Graph::default();

    graph.nodes.push(Node {
        label: NodeLabel::Artifact,
        id: hex.clone(),
        props: vec![
            ("kind", format!("{:?}", artifact.manifest.artifact_kind)),
            ("regime_id", regime.clone()),
        ],
    });
    graph.nodes.push(Node {
        label: NodeLabel::Regime,
        id: regime.clone(),
        props: vec![],
    });
    graph.edges.push(Edge {
        label: EdgeLabel::InRegime,
        from: hex.clone(),
        to: regime,
        props: vec![],
    });

    // Attestation nodes: one per appended attestation, ATTESTED_BY from the
    // artifact. Type + signer ride as properties so the kind-policy query
    // (negative control (b)) can interrogate the set.
    for (i, att) in artifact.attestations.iter().enumerate() {
        let att_id = format!("{hex}:att:{i}");
        graph.nodes.push(Node {
            label: NodeLabel::Attestation,
            id: att_id.clone(),
            props: vec![
                ("type", format!("{:?}", att.attestation_type)),
                ("signer_key_id", att.key_id.clone()),
            ],
        });
        graph.edges.push(Edge {
            label: EdgeLabel::AttestedBy,
            from: hex.clone(),
            to: att_id,
            props: vec![],
        });
    }

    if let ArtifactPayload::Rules(rules) = &artifact.payload {
        for rule in rules {
            let rule_id = rule_node_id(&hex, &rule.rule_id);
            graph.nodes.push(Node {
                label: NodeLabel::Rule,
                id: rule_id.clone(),
                props: vec![("rule_id", rule.rule_id.clone())],
            });
            graph.edges.push(Edge {
                label: EdgeLabel::Contains,
                from: hex.clone(),
                to: rule_id.clone(),
                props: vec![],
            });
            push_corpus_doc(&mut graph, &rule.source.document_id);
            // Article/section granularity is recorded on the DocumentRef and
            // rides the edge — the blast-radius query keys off it.
            let mut props = vec![];
            if let Some(article) = &rule.source.article {
                props.push(("article", article.clone()));
            }
            if let Some(section) = &rule.source.section {
                props.push(("section", section.clone()));
            }
            graph.edges.push(Edge {
                label: EdgeLabel::Cites,
                from: rule_id,
                to: rule.source.document_id.clone(),
                props,
            });
        }
        // SPANS edges from the artifact's span index (rule → spanned doc,
        // span locator detail as edge properties).
        for entry in &artifact.source_span_index.entries {
            let rule_id = rule_node_id(&hex, &entry.rule_id);
            for span in &entry.spans {
                push_corpus_doc(&mut graph, &span.document_id);
                let mut props = vec![];
                if let Some(article) = &span.article {
                    props.push(("article", article.clone()));
                }
                if let Some(section) = &span.section {
                    props.push(("section", section.clone()));
                }
                graph.edges.push(Edge {
                    label: EdgeLabel::Spans,
                    from: rule_id.clone(),
                    to: span.document_id.clone(),
                    props,
                });
            }
        }
    }

    if let ArtifactPayload::IntentSpec(spec) = &artifact.payload {
        let intent_id = format!("{hex}:intent:{}", spec.action_class);
        graph.nodes.push(Node {
            label: NodeLabel::IntentSpec,
            id: intent_id.clone(),
            props: vec![
                ("action_class", spec.action_class.clone()),
                ("criteria_count", spec.criteria.len().to_string()),
            ],
        });
        graph.edges.push(Edge {
            label: EdgeLabel::Contains,
            from: hex.clone(),
            to: intent_id.clone(),
            props: vec![],
        });
        // IntentSpec spans travel on the payload (the rule-oriented span
        // index is empty for this kind by construction — assemble_payload).
        for span in &spec.source_spans {
            push_corpus_doc(&mut graph, &span.document_id);
            let mut props = vec![];
            if let Some(article) = &span.article {
                props.push(("article", article.clone()));
            }
            if let Some(section) = &span.section {
                props.push(("section", section.clone()));
            }
            graph.edges.push(Edge {
                label: EdgeLabel::Spans,
                from: intent_id.clone(),
                to: span.document_id.clone(),
                props,
            });
        }
    }

    graph
}

/// `CONFLICTS_WITH` edges for one artifact's rule payload, by re-running the
/// same [`ke_compiler::verify::verify`] the publish gate ran over the decoded
/// signed payload. T4 conflicts are **intra-artifact** (the gate verifies one
/// compile's rule set), so both endpoints are scoped to `artifact_hex`.
/// Symmetric conflicts are emitted once, endpoints sorted, so the edge list is
/// deterministic and rerun-stable.
pub fn conflict_edges(artifact_hex: &str, rules: &[ke_core::ir::RuleIR]) -> Vec<Edge> {
    let report = ke_compiler::verify::verify(rules);
    report
        .conflicts
        .iter()
        .map(|conflict| {
            let mut ids = conflict.rule_ids.clone();
            ids.sort();
            let (a, b) = (&ids[0], &ids[ids.len() - 1]);
            Edge {
                label: EdgeLabel::ConflictsWith,
                from: rule_node_id(artifact_hex, a),
                to: rule_node_id(artifact_hex, b),
                props: vec![
                    ("class", format!("{:?}", conflict.class)),
                    ("severity", format!("{:?}", conflict.severity)),
                ],
            }
        })
        .collect()
}

/// One edge as a canonical JSON value (label name + endpoints + props object).
fn edge_json(edge: &Edge) -> serde_json::Value {
    let props: serde_json::Map<String, serde_json::Value> = edge
        .props
        .iter()
        .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.clone())))
        .collect();
    serde_json::json!({
        "label": edge.label.name(),
        "from": edge.from,
        "to": edge.to,
        "props": props,
    })
}

/// Recompute the full edge set (extraction + conflict recompute) over every
/// golden under `<fixtures_root>/artifacts/*/artifact.kew`, as the canonical
/// JSON the committed fixture pins (ADR-0023 D4). Deterministic: goldens are
/// walked in sorted directory order; edge order is extraction order (a pure
/// function of the signed bytes).
pub fn golden_edges_json(fixtures_root: &str) -> anyhow::Result<serde_json::Value> {
    let artifacts_dir = std::path::Path::new(fixtures_root).join("artifacts");
    let mut dirs: Vec<_> = std::fs::read_dir(&artifacts_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("artifact.kew").exists())
        .collect();
    dirs.sort();

    let mut goldens = serde_json::Map::new();
    for dir in dirs {
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let kew = std::fs::read(dir.join("artifact.kew"))?;
        let (artifact, _) = ke_artifact::decode_artifact(&kew)
            .map_err(|e| anyhow::anyhow!("decode {name}: {e}"))?;
        let hex = hash_hex(&artifact.manifest.artifact_hash);

        let mut edges = extract_graph(&artifact).edges;
        if let ArtifactPayload::Rules(rules) = &artifact.payload {
            edges.extend(conflict_edges(&hex, rules));
        }

        goldens.insert(
            name,
            serde_json::json!({
                "artifact_hash": hex,
                "edges": edges.iter().map(edge_json).collect::<Vec<_>>(),
            }),
        );
    }
    Ok(serde_json::json!({ "goldens": goldens }))
}

// ---------------------------------------------------------------------------
// Oracles (ADR-0023 D6): the Rust side of each Cypher differential. Pure
// functions of the exported graph; semantics chosen to be exactly decidable
// on both sides (plain reachability + a single hop — no path predicates).
// ---------------------------------------------------------------------------

/// Blast radius of amending `document_id` (optionally one `article` of it):
/// every rule/IntentSpec citing or spanning it, every artifact containing one
/// of those, and every attestation downstream of those artifacts. Sorted,
/// deduplicated node ids.
pub fn blast_radius(graph: &Graph, document_id: &str, article: Option<&str>) -> Vec<String> {
    let cites = |edge: &Edge| {
        (edge.label == EdgeLabel::Cites || edge.label == EdgeLabel::Spans)
            && edge.to == document_id
            && article.is_none_or(|a| edge.props.iter().any(|(k, v)| *k == "article" && v == a))
    };
    let citing: Vec<&String> = graph
        .edges
        .iter()
        .filter(|e| cites(e))
        .map(|e| &e.from)
        .collect();

    let mut out: Vec<String> = citing.iter().map(|s| s.to_string()).collect();
    let artifacts: Vec<&String> = graph
        .edges
        .iter()
        .filter(|e| e.label == EdgeLabel::Contains && citing.contains(&&e.to))
        .map(|e| &e.from)
        .collect();
    out.extend(artifacts.iter().map(|s| s.to_string()));
    out.extend(
        graph
            .edges
            .iter()
            .filter(|e| e.label == EdgeLabel::AttestedBy && artifacts.contains(&&e.from))
            .map(|e| e.to.clone()),
    );
    out.sort();
    out.dedup();
    out
}

/// Cross-regime conflict exposure: rules in `target_regime` that are one
/// `CONFLICTS_WITH` hop away from the **citation closure** of
/// `source_regime`'s rules (undirected BFS over CITES/SPANS through shared
/// documents). Sorted, deduplicated rule node ids.
pub fn conflict_exposure(graph: &Graph, source_regime: &str, target_regime: &str) -> Vec<String> {
    use std::collections::{HashMap, HashSet, VecDeque};

    // artifact hex → regime, and rule/intent node → its artifact.
    let regime_of: HashMap<&str, &str> = graph
        .edges
        .iter()
        .filter(|e| e.label == EdgeLabel::InRegime)
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect();
    let artifact_of: HashMap<&str, &str> = graph
        .edges
        .iter()
        .filter(|e| e.label == EdgeLabel::Contains)
        .map(|e| (e.to.as_str(), e.from.as_str()))
        .collect();
    let in_regime = |node: &str, regime: &str| {
        artifact_of
            .get(node)
            .and_then(|a| regime_of.get(a))
            .is_some_and(|r| *r == regime)
    };

    // Undirected citation adjacency (rules/intents ↔ documents).
    let mut adjacent: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &graph.edges {
        if edge.label == EdgeLabel::Cites || edge.label == EdgeLabel::Spans {
            adjacent
                .entry(edge.from.as_str())
                .or_default()
                .push(edge.to.as_str());
            adjacent
                .entry(edge.to.as_str())
                .or_default()
                .push(edge.from.as_str());
        }
    }

    // BFS from every source-regime rule.
    let mut reached: HashSet<&str> = HashSet::new();
    let mut queue: VecDeque<&str> = graph
        .nodes
        .iter()
        .filter(|n| n.label == NodeLabel::Rule && in_regime(&n.id, source_regime))
        .map(|n| n.id.as_str())
        .collect();
    reached.extend(queue.iter().copied());
    while let Some(node) = queue.pop_front() {
        for next in adjacent.get(node).into_iter().flatten() {
            if reached.insert(next) {
                queue.push_back(next);
            }
        }
    }

    // One conflict hop off the closure, landing in the target regime.
    let mut out: Vec<String> = graph
        .edges
        .iter()
        .filter(|e| e.label == EdgeLabel::ConflictsWith)
        .flat_map(|e| {
            [(&e.from, &e.to), (&e.to, &e.from)]
                .into_iter()
                .filter(|(b, c)| reached.contains(b.as_str()) && in_regime(c, target_regime))
                .map(|(_, c)| c.clone())
                .collect::<Vec<_>>()
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Add a `CorpusDoc` node once per document id.
fn push_corpus_doc(graph: &mut Graph, document_id: &str) {
    if !graph
        .nodes
        .iter()
        .any(|n| n.label == NodeLabel::CorpusDoc && n.id == document_id)
    {
        graph.nodes.push(Node {
            label: NodeLabel::CorpusDoc,
            id: document_id.to_string(),
            props: vec![],
        });
    }
}
