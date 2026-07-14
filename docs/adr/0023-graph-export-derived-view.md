# 0023. Graph export — a verify-gated, derived read-only view of the artifact substrate (Neo4j)

**Status:** Proposed — **BUILT 2026-07-14** (trigger fired by Hossain; differential harness ran GREEN live, `passed=11 failed=0`; acceptance = PR merge). See § Build-time amendments.
**Date:** 2026-07-13
**Spec references:** § 5 (authority boundaries), § 6 (non-authoritative surfaces), § 12 (T4 conflicts), § 14 (consumer surface)
**Related ADRs:** 0005 (T4 conflict classes), 0016 (consumer-agnostic verify + `ArtifactProvenance`), 0019 (consumer trust boundary: re-derive trust, non-`published` blocked, fail closed on `unknown`), 0020 (frontend rewire deferred; `GraphVisualizer` off the artifact path), 0021 (IntentSpec kind + `artifact_kind` on provenance), 0022 (kind-aware R7 co-attestation).
**Gate:** none. This is deliberately **not** a migration gate — no canon change, no
golden re-pins, no new artifact kind. It is the mirror-image of ADR-0021's blast
radius: 0021 paid a one-time canonicalization bump to make the substrate
polymorphic; 0023 adds a derived view over it and must not touch the substrate
at all.

## Context

The verified substrate is already a property graph stored as content-addressed
artifacts: rules reference premises; source spans point into corpus documents;
the T4 conflict gate implies rule↔rule edges; typed attestations link experts
to artifact hashes; jurisdictions group corpora; artifact hashes form a lineage
DAG. ADR-0021 adds a second node family — IntentSpec action classes with
authorization criteria — and ADR-0022 makes the attestation edge set
kind-dependent. Nothing new has to be modeled; it has to be **materialized**.

The motivation is external and honest about being so: graph-database
experience recurs as a "plus" in the roles this portfolio targets, and the gap
should be closed with a real, differential-tested artifact rather than a thin
claim. ATLAS is the right host because only here does a graph view inherit a
verification story instead of needing one invented.

Three facts found during design shape the decision:

1. **The T4 conflict report is not persisted.** `ke-compiler::verify` computes
   it at compile/publish time for gating and it is discarded — `ke-artifact`
   has no conflict content, and `compile.rs` only prints blocking conflicts
   into the error message. "Export the recorded conflict output" is therefore
   not an available option; there is no recorded output.
2. **ADR-0021 § 6 gives an exporter a signed-boundary entry point.**
   `artifact_kind` lands on `ArtifactProvenance`, so a consumer of the folded
   `verify_artifact` output can discriminate kinds without bespoke payload
   spelunking.
3. **The frontend already has a `GraphVisualizer` page**, one of the eight
   off-artifact-path ML pages whose rewire ADR-0020 deferred. This ADR must
   not be confused with it (see § Positioning).

### Invariant to protect

The deterministic tree-walk engine executing signed artifacts remains the
**only decision path**. The graph store is a derived, read-only view — never
an input to a decision, never a runtime backend. This keeps the
determinism/audit thesis intact and makes the extension honest by
construction.

## Decision

Build **approach B** from the pre-decision analysis (exporter + differential
graph queries), shaped as follows. Approaches A (exporter only) and C (graph
store as runtime backend) are recorded under Alternatives.

1. **Verify-gated consumer.** The exporter is a `ke-cli graph export`
   subcommand that behaves as a consumer under **ADR-0019 discipline**: it
   runs the folded `verify_artifact` per artifact and exports **only verified
   + `published`** artifacts. Non-`published` is blocked even with valid
   crypto; `unknown` fails closed. The exporter also **re-addresses** each
   artifact — recomputes the content hash from the decoded bytes and compares —
   before emitting nodes. Refused artifacts appear as refusals in the export
   log — never as nodes. This makes the exporter the **third** consumer under
   the discipline, not the first: COMPASS (WASM verifier, ADR-0019) and the
   tic treasury resolver (`KeArtifactResolver`: folded
   `ke_artifact_py.verify_artifact` per hash, fail-closed on absent hash,
   rejected verdict, or re-address mismatch — live-verified 2026-07-12)
   already obey it. The graph inherits an existing trust boundary instead of
   inventing one.
2. **Schema.** Nodes: `Rule`, `Premise`, `SourceSpan`, `CorpusDoc`,
   `Jurisdiction`, `Attestation`, `IntentSpec`. Edges: `CITES`, `SPANS`,
   `CONFLICTS_WITH`, `ATTESTED_BY`, `SUPERSEDES`. `ATTESTED_BY` is
   **kind-dependent per ADR-0022**: rule-shaped kinds carry the
   `{ScenarioCoverage, SourceFidelity}` co-attestation pair; `IntentSpec`
   carries `SourceFidelity` only. The asymmetry is deliberately queryable
   (see negative control (b)).
3. **IntentSpec is in v1, with a hard dependency stated.** The v1 schema
   includes IntentSpec nodes and their kind-dependent attestation edges. The
   build depends on ADR-0021 and ADR-0022 being **Accepted and merged**; if
   the trigger fires first, the build waits on those PRs, not the other way
   around. The two-node-family graph — rules *and* authorization criteria
   sharing spans and attestations — is the differentiated story, and it is no
   longer speculative: the full treasury payment loop consuming the signed
   IntentSpec ran green end-to-end on 2026-07-12, so the second node family
   has a live consumer and a golden that genuinely passes folded verify
   (post-ADR-0022 fix).
4. **`CONFLICTS_WITH` by deterministic recompute.** The exporter links
   `ke-compiler` and re-runs `verify()` over the **decoded signed payload** at
   export time. Honesty argument: `verify()` is a pure, deterministic function
   of the signed bytes, so the recompute is identical to what the gate saw at
   publish time — recompute ≡ recorded, because the input is content-addressed
   and the function is pure. Pinned by a fixture test: conflict edges
   recomputed over the golden artifacts must match a committed expected-edges
   fixture, so drift in the conflict gate turns the export test red instead of
   silently reshaping the graph. (The fixture is generated by tooling, never
   hand-edited, per the `fixtures/` rule.)
5. **Engine: Neo4j Community via docker-compose.** Recognized name, Cypher is
   the transferable skill, zero licensing friction at this scale. Kùzu
   (embedded, nicer dev loop, no recognition value) and Neptune (AWS
   cost/setup for nothing at portfolio scale) rejected.
6. **Two differential queries, each with a Rust-side oracle** — the house
   discipline (same shape as the Rust↔Python 1,326-scenario equivalence
   harness):
   - *Cross-jurisdiction conflict reachability* ("which EU rules transitively
     conflict with any US stablecoin rule?") — oracle: transitive closure over
     the recomputed T4 pairwise conflicts, computed in Rust.
   - *Premise blast-radius* ("if this source span is amended, which rules and
     attestations are downstream?") — oracle: a span-index walk in Rust.
7. **Negative controls and a vacuity guard:**
   - (a) a known-bad **twin export** with one mutated edge — the conflict
     reachability differential must go red;
   - (b) a twin export with a **doctored attestation set** — a Cypher query
     for "artifacts whose attestation set violates their kind's policy"
     (ADR-0022 semantics) must return non-empty. Control (b) is what makes the
     kind-aware pivot visible in the graph rather than trivia.
   - (c) **vacuity guard**: every differential asserts **both sides are
     non-empty** over the golden corpus before comparing — an all-refused
     export, or an oracle that returns nothing, is a **red** run, never a
     trivially-equal green one. Motivated by the in-house R7 incident
     (ADR-0022 § Context): the gap stayed latent precisely because an
     equality-shaped test compared three *identical rejections* and reported
     green. Equality tests must prove they compared something.
8. **Harness placement: script, non-gating in CI.** The differential harness
   is `scripts/graph-differential.sh` (bash, per repo shell policy), run on
   demand — the Playwright precedent for experimental/non-gating tooling. CI
   gains no Neo4j/docker service dependency. The **pure-Rust fixture test**
   for conflict-edge recompute (Decision 4) does run in ordinary `cargo test`.
   The claim gate is procedural: the PR that closes the trigger does not merge
   until the harness's green run is recorded in it.

### Positioning — what this is not

Not a frontend feature, and specifically **not a rewire of the existing
`GraphVisualizer` page**: that page is off the artifact path and ADR-0020's
deferral stands untouched. The Neo4j view is analyst-side tooling reached by
`ke-cli graph export` + docker-compose, outside the frontend entirely.

Also not a cross-repo trace graph: the live payment loop's append-only
`ACHIEVED` log and settlement records (tic / COMPASS Stage C(b)) carry
`intent_spec_hash` and `trajectory_hash`, which would join this graph to
runtime lineage. That is a possible future extension owned by those repos —
not a v1 export input, and adding it would require its own trust-boundary
argument (those records are runtime state, not signed artifacts).

### Trigger, cap, and kill signal

Build the week a live conversation prices graph experience above "plus" (a
recruiter screen raises it, or an interviewer foreshadows it). Until then this
ADR stays Proposed and the work stays behind higher-priority gaps. If built
speculatively anyway, cap at the **2–3 day** bound; schedule overrun is a kill
signal, not a reason to extend.

### Honesty guardrails

Nothing from this ADR is claimable until the build is **merged and its
differential harness green**. The claim ceiling even then is: *"built and
differential-tested a graph view over a rule-assurance engine (Neo4j/
Cypher)"* — never "production graph database experience," never retro-attached
to earlier work. Pre-existing March-era "executable knowledge graphs /
Node2Vec" claims stay quarantined until repo-verified; this work neither
validates nor depends on them.

## Consequences

- **Desirable.** The graph inherits the verify story by construction
  (fail-closed exporter); the substrate is untouched (no canon bump, no golden
  churn, no gate); the differential harness turns "graph experience" from a
  keyword into a defensible artifact; the ADR-0022 kind-asymmetry becomes
  demonstrable rather than buried in `verify_attestation_set`.
- **Undesirable / accepted.** The exporter links `ke-compiler` (a heavier
  dependency than a pure `ke-artifact` walk) — the price of deterministic
  conflict recompute. The Cypher differentials are not CI-gated; regressions
  there surface only on manual runs. Neo4j via docker is a local-infra
  dependency the repo otherwise avoids.
- **Authority unchanged.** The graph is read-only and derived; it cannot sign,
  attest, publish, or influence resolution. Verify-only surfaces stay
  verify-only (spec § 6).

## Alternatives considered

- **A: exporter only, no queries.** Rejected as the end state: invites "so
  what did you *query*?" — the differential queries are what convert the
  export into a claim. (A is subsumed as B's first milestone.)
- **C: graph store as a runtime backend.** Rejected: couples a
  nondeterministic external store into the decision path, breaks the
  content-addressing story, weeks of work for no additional claim value.
- **Persist the T4 conflict report in the envelope** (to make `CONFLICTS_WITH`
  truly "recorded"). Rejected: a second canonicalization bump re-pinning every
  golden — exactly the blast radius ADR-0021 paid once and this ADR must not
  pay again — to record derived compiler output inside the artifact it was
  derived from. Deterministic recompute delivers the same honesty for free.
- **Raw `ke-artifact` decode without the verify gate.** Rejected: the graph
  would materialize unverified or revoked content, weakening "derived from the
  verified substrate" to "derived from whatever is on disk."
- **Trace replay via existing `ke-cli` commands.** Rejected: couples the
  exporter to CLI output formats and gives no payload access for the
  premise/span edges that make the graph interesting.
- **Gating CI job with a Neo4j service container.** Rejected for now: adds a
  docker service and minutes to every CI run for a portfolio-scale tool that
  changes rarely. Revisit only if the graph view acquires a real downstream
  consumer.

## Build-time amendments (2026-07-14)

Recon against the real substrate amended the § Decision-2 vocabulary — every
delta is the same shape as the `CONFLICTS_WITH` finding: **no recorded source,
so the exporter drops or renames rather than fabricates.**

- **`Jurisdiction` → `Regime`.** The manifest records `regime_id`; a
  jurisdiction entity exists nowhere. Inferring "EU" from `mica_*` naming
  would be exporter-side fabrication. The flagship query is cross-**regime**
  conflict exposure.
- **`SUPERSEDES` dropped** (resolves former open question 2). The registry
  records deprecation with a free-text reason only — no structured hash→hash
  lineage. Add the edge if/when the registry records one.
- **`Premise` dropped.** Conditions live inside `decision_tree`; the recorded
  citation substrate is `DocumentRef` + spans. Rule-side granularity rides
  `CITES` edge properties (`article`, `section`) — the rule corpus records
  **zero node-level spans**, so a rule-side `SPANS` family would be vacuous.
  `SPANS` edges exist where spans are recorded: the IntentSpec payload.
- **Conflict edges are intra-artifact** (T4 verifies one compile's rule set),
  so the exposure query is defined as **citation-closure then one conflict
  hop** — chosen because plain BFS reachability and Cypher var-length trail
  reachability provably agree, where a path-predicate formulation would let
  the two sides legally disagree (relationship-reuse semantics).
- **Re-address is a distinct check, proven non-vacuous:** a fully-valid
  artifact planted at another artifact's address passes folded verify
  entirely; only address ≡ manifest-hash refuses it
  (`export_refuses_valid_artifact_at_wrong_address`). Implemented via a new
  `RegistryBackend::list_addresses` (addresses = what the store files under,
  vs `list_manifests` = what the bytes claim).
- **Fixture pin widened:** `fixtures/graph/expected_edges.json` (generated
  only by `gen-graph-fixture`) pins the **full** edge set over the goldens,
  not just conflicts — the goldens are small clean packs whose conflict set
  is legitimately empty, and an all-empty pin would be vacuous. Regeneration
  on an unchanged tree is a byte-identical no-op (verified).
- **Output format** (resolves former open question 1): generated
  **idempotent MERGE-only Cypher** (`graph.cypher`) + `refusals.log`, loaded
  via `cypher-shell`. Re-loading the same export is a no-op — the read-side
  mirror of exporter determinism.

**Evidence (2026-07-14, this machine):** `cargo test --workspace` 187/0
(11 new graph tests, each watched RED first — TDD); fmt + clippy clean;
`scripts/graph-differential.sh` GREEN `passed=11 failed=0` — both Cypher
differentials byte-equal to the Rust oracles under the vacuity guard, the
kind-policy query empty on the honest export, and both negative controls
**detected** (mutated conflict edge broke the differential; doctored
attestation type tripped the policy query). Registry under test: the two
harness packs + `mica_stablecoin` + an authored IntentSpec (two-node-family
graph live) + one deliberately unpublished artifact named in the refusal log.

## Open questions — all resolved

1. **Exporter output format — RESOLVED at build:** generated idempotent
   Cypher (see § Build-time amendments).
2. **`SUPERSEDES` edge source — RESOLVED at build:** no recorded source
   exists; edge dropped from v1 (see § Build-time amendments).
3. **ADR-0021 status drift — RESOLVED 2026-07-13.** 0021 and 0022 are stamped
   Accepted (PRs #12/#14 merged to main; index rows updated; the
   attestation-schema § 6B/§ 7 amendment required by 0022's acceptance is
   applied). This ADR's Decision-3 dependency was satisfied before the build.
