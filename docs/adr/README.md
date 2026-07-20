# Architecture Decision Records

Use this directory for ADRs that capture decisions which are referenced
elsewhere in the spec or codebase. Each ADR is one Markdown file named
`NNNN-short-slug.md` where `NNNN` is a zero-padded sequence number.

Suggested template:

```markdown
# NNNN. Short title

**Status:** Proposed | Accepted | Superseded by NNNN
**Date:** YYYY-MM-DD
**Spec references:** § X.Y, § A.B

## Context
What problem motivated the decision; what constraints applied.

## Decision
What was decided, in one or two paragraphs.

## Consequences
What follows from the decision — both desirable and undesirable.

## Alternatives considered
What else was on the table and why those options lost.
```

## Index

Accepted (Gate 1) — together these three form the canonical encoding profile
that `docs/canonical-encoding.md` documents in prose:

- [0001 Jurisdiction time-zone representation](0001-jurisdiction-time-zone.md) — spec § 8.4
- [0002 Canonical wire codec — postcard](0002-canonical-codec-postcard.md) — spec § 8
- [0003 Decimal scalar representation — mantissa/scale](0003-decimal-scalar-representation.md) — spec § 8

Gate 2:

- [0004 Source-span coverage policy (T1) and span/provenance separation](0004-source-span-coverage-policy.md) — spec § 11 — **Accepted**
- [0005 T4 conflict classes and severities for Gate 2](0005-t4-conflict-classes-gate-2.md) — spec § 12 — **Accepted** (note: the file carries a later "shared scenario" amendment that is still **Proposed** — needs domain-reviewer sign-off; the original policy block is Accepted and unchanged)
- [0006 `effective_window` is optional (amends Gate 1 IR)](0006-effective-window-optional.md) — spec § 8.4 — **Accepted**

Gate 3:

- [0007 Effective windows in the preview runtime (tz optional; `[from,to)` preview-only)](0007-effective-window-preview-runtime.md) — spec § 8.4 — **Accepted**
- [0008 Execution equivalence boundary and `FactValue` representation](0008-execution-equivalence-boundary.md) — spec § 20 — **Accepted**

Gate 4 (prerequisites — **Accepted** by Hossain 2026-06-11; see
`dev/briefs/gate-4-artifact-registry-attestation.md` §2):

- [0009 Expert key authority, key lifecycle, and revocation behavior](0009-expert-key-authority-and-revocation.md) — spec § 21.1, § 21.6, § 20 — **Accepted**
- [0010 Trusted timestamp authority for typed attestations](0010-trusted-timestamp-authority.md) — spec § 21.5, § 10 — **Accepted**
- [0011 T2/T3 publication policy + sidecar deployment](0011-t2t3-publication-policy-and-sidecar-deployment.md) — spec § 21.2, § 21.3, § 11 — **Accepted**
- [0012 S3 registry layout + PEP 503 package index layout](0012-s3-registry-and-pep503-index-layout.md) — spec § 21 (resolved persistence), § 14 — **Accepted**
- [0013 Revocation policy reconciliation (§15) + rollback-target eligibility](0013-revocation-policy-reconciliation.md) — spec § 15 — **Accepted** (authorizes canon bump 0.3.0→0.4.0 / ke-canon-3→ke-canon-4, pending execution)
- [0014 Audit/observability contract (§18) ownership + pre-freeze field model](0014-audit-contract-ownership.md) — spec § 18 — **Accepted**
- [0015 Temporal orchestration ownership: orchestration stays Python, the work moves to Rust](0015-temporal-orchestration-ownership.md) — spec § 2 (non-goals), § 14, § 15, § 19 (Gate 6) — **Proposed** (restates existing spec policy)
- [0016 Phase 4 is consumer-agnostic verification + provenance export, with both bindings](0016-phase4-consumer-agnostic-verification.md) — spec § 6, § 14, § 16 — **Accepted** (sign-off by Hossain, 2026-06-13; rescopes the brief's Phase 4; pulls `ke-wasm` verify into Gate 4; 4a = pure Rust core delivered, 4b = PyO3/WASM/contract-test)

Gate 5:

- [0017 Platform-api decoupled; Gate-4 C1/C2 redefined; Gate-5 proceeds](0017-gate5-sequencing-atlas-surfaces-independent.md) — spec § 19, § 14, § 16, § 6 — **Accepted** (sign-off by Hossain, 2026-06-15; platform-api is not the consumer — COMPASS is; C1 verifier + C2 equivalence foundation MET in-repo, consumer integration deferred to the post-Gate-5 COMPASS rewire; 5d/5e stay gated)
- [0018 `ke serve` uses SSE (not WebSocket) and is strictly non-authoritative](0018-serve-transport-sse-and-non-authoritative-scope.md) — spec § 16, § 7.4, § 6, § 5/§10/§13 — **Accepted** (sign-off by Hossain, 2026-06-15; windows-gnu can't build WebSocket/tokio deps → tiny_http + SSE; serve never signs/publishes)
- [0019 Agent-identity governance framing + COMPASS federated-consumer trust boundary](0019-agent-identity-governance-framing-and-consumer-trust-boundary.md) — spec § 5/§10/§13, § 9, § 14/§16, § 20 — **Accepted** (sign-off by Hossain, 2026-06-15; re-decides nothing in 0009/0013; adopts agent-identity vocabulary as the audit lens — lifecycle-as-control-point, credential ≠ authority — and binds the consumer rule: COMPASS re-derives trust, treats non-`published` as blocked even with valid crypto, fails closed on `unknown`)
- [0020 Gate-5 frontend rewire: G5-5 deferred — COMPASS is the consumer](0020-gate5-frontend-rewire-honest-acceptance.md) — spec § 19 (G5-5), § 7.4, § 13, § 16, § 6 — **Accepted** (sign-off by Hossain, 2026-06-18; 🟡 **DEFER** the ATLAS frontend rewire onto local Rust surfaces — low-value post-ADR-0017 because the **real consumer is COMPASS**, which verifies the ATLAS artifact path in-browser via the `@platform/atlas-artifact` WASM verifier, gated post-Gate-5; ATLAS's own React frontend is producer-side authoring/review tooling, and most pages — ML/analytics/jurisdiction/credit — are off the artifact path; the KEWorkbench WASM preview pane + 5e review components stay in-tree behind default-off flags as inert affordances, not a delivered rewire; the engine surfaces it would have consumed — 5a `ke serve`, 5b-preview WASM bindings, 5b-data `.kew` export/import (G5-4 MET), 5c lint — stand on their own and are **not** deferred)

Treasury / IntentSpec (a canonicalization-boundary change; landed via PRs
#12–#14 without a formal gate number ever being assigned):

- [0021 IntentSpec artifact kind — polymorphic envelope payload for non-rule artifacts](0021-intentspec-artifact-kind-polymorphic-payload.md) — spec § 8.1, § 8.2, § 8.3, § 14 — **Accepted** (sign-off by Hossain; decision merged via PR #12, implementation — canon-5, all goldens regenerated — via PR #13; amends § 8.2's four-kind list; makes `compiled_ir` a polymorphic `ArtifactPayload` sum type so non-rule kinds — IntentSpec first, then EquivalenceMatrix/TestCorpus/PolicyBundle — have an envelope representation; breaking canon bump, all goldens regenerated; adds kind↔payload dispatch + kind-aware attestation policy + `artifact_kind` on the provenance; gate number still unassigned)
- [0022 Kind-aware R7 co-attestation — an IntentSpec approval co-attests with SourceFidelity only](0022-intentspec-r7-coattestation.md) — spec § 10, attestation-schema § 6B/§ 7 — **Accepted** (sign-off by Hossain, 2026-07-13; merged via PR #14, 26 checks green; fixes the latent contradiction where kind-aware R6 admitted the IntentSpec two-type set but unconditional R7 rejected it, making every IntentSpec unverifiable; `co_attestation_types(&ArtifactKind)` selector, two pinning tests, both arms mutation-verified non-vacuous; schema doc § 6B/§ 7 amended at acceptance)

Derived views (no gate — substrate untouched):

- [0023 Graph export — a verify-gated, derived read-only view of the artifact substrate (Neo4j)](0023-graph-export-derived-view.md) — spec § 5, § 6, § 12, § 14 — **Accepted** (merged 2026-07-15, PR #16 — the acceptance criterion was the merge itself; built 2026-07-14, trigger fired; PR records a fresh 187/0 + fmt/clippy + harness-GREEN re-verification; exporter is the third ADR-0019-disciplined consumer after COMPASS and the tic resolver — verified+`published` only, fail-closed, re-addressed via new `list_addresses`; `CONFLICTS_WITH` by deterministic recompute of the unpersisted T4 report, pinned by the tool-generated `fixtures/graph/expected_edges.json`; build-time schema amendments recorded in the ADR: `Regime` not `Jurisdiction`, `SUPERSEDES`/`Premise` dropped (no recorded source), article-granular `CITES`; `ke graph export|oracle-*` + `scripts/graph-differential.sh` ran GREEN live `passed=11 failed=0` — both Cypher↔Rust differentials byte-equal under vacuity guards, both negative controls detected; workspace 187/0, fmt/clippy clean; not a rewire of the off-path `GraphVisualizer` page — ADR-0020 stands)

Gate 6 (reconciled — the platform-cutover spec scope is unmeetable post-ADR-0017):

- [0024 Gate-6 scope reconciliation: revocation runtime-decision + registry surface completion; platform cutover deferred](0024-gate6-scope-reconciliation.md) — spec § 19 (Gate 6), § 15, § 14, § 18, § 21 — **Accepted** (merged 2026-07-19, PR #17 — acceptance was the merge itself, per the 0023 precedent; authored 2026-07-19 from [`dev/briefs/gate-6-plan-and-next-session-seed.md`](../../dev/briefs/gate-6-plan-and-next-session-seed.md); accepts 0015 in the same change; delivers the ADR-0009 § 4 reason-class → policy decision as pure `ke_core::revocation` — stricter-of(floor, configured), floor never lowerable — wired into `ke revoke --reason-class` with the legacy path byte-compatible; completes `serve /resolve?regime=&effective=`; surfaces the revocation sidecar on `ResolutionRecord`+`VerifyResponse` exactly when Revoked; verify stays fail-closed; platform Temporal pinning / Python KE removal / Rust Temporal worker deferred — no orchestrator consumer exists)

Production readiness (corporate agentic pivot — `dev/briefs/2026-07-21-intent-authoring-planes-scope.md`):

- [0025 Production key authority: custody split by signer role, IdP-backed tenant attesters, compromise scope](0025-production-key-authority.md) — spec § 20, § 21.1, § 21.6, § 10, § 9 — **Proposed** (authored 2026-07-21; closes ADR-0009's two post-acceptance open items: custody decided per role — registry root/compiler on KMS/HSM per 0009 § 3, tenant attesters IdP-backed with hardware-token alternative; compromise = retroactive for trust, prospective for executed history with exposure enumerated via the 0023 graph exporter; non-local policy REJECTS test keys — the ADR's one code deliverable; ed25519 pinned, custody adapts to scheme never the reverse; depends on ADR-0010 TSA operationalization, named not solved)

Anticipated (later gates — numbers assigned when authored):

- Package-manager choice (spec § 21.9) — only if pnpm is later adopted
- Frontend visual-regression tooling (spec § 21.8) — Playwright self-hosted chosen for 5d as **experimental / non-gating** (Linux-CI-canonical baselines); no formal ADR unless it is promoted to a required gate

## How the ADRs tie together

The 24 accepted-or-proposed decisions form five clusters, drawn from which
ADR cites which (spot-checked against the files, not asserted as an
exhaustive graph). One node is genuinely isolated: **0004** (source-span
coverage) is cited by no other ADR — it binds through spec § 11 and Gate 2
directly.

**1. The canon profile (0001–0003, amended by 0006/0007/0013/0021).**
Everything rests here. **0002 (postcard codec) is the most-cited ADR in the
set** — its byte-layout rules are why 0009's signatures, 0012's
content-addressed registry, 0013's discriminant reordering, and 0021's payload
sum type all had to reason about canonicalization bumps. The current triplet
is `0.5.0` / `postcard-1` / `ke-canon-5` (`crates/ke-core/src/version.rs`);
each historical bump is an ADR: canon-2 = 0006, canon-3 = 0007, canon-4 =
0013, canon-5 = 0021.

**2. The trust chain (Gate 4: 0009–0014, 0016; plus 0008 carried forward).**
Two hubs: **0009** (expert key authority — who may sign, how keys die) and
**0012** (S3 registry + PEP 503 index — where trust is stored). 0010
(timestamps), 0011 (publication policy), 0013 (revocation), 0014 (audit
contract) hang off them — and 0008's Gate-3 execution-equivalence boundary is
carried into this cluster by 0010 and 0012, which both cite it; **0016**
turns the whole chain into a consumer-agnostic `verify_artifact` fold with
`ArtifactProvenance` as its export — the single surface every consumer below
calls. **0024** (Gate 6) closes the chain's last open loop: it turns 0009 § 4's
reason-class table into an executable decision and surfaces the 0013 revocation
record at the resolve/verify boundary — without touching what 0016 verifies.

**3. The consumer boundary (0017–0020).** 0017 names the consumer (COMPASS,
platform-api decoupled) → 0018 shapes the serve transport → **0019 states the
discipline** (re-derive trust; non-`published` blocked even with valid
crypto; fail closed on `unknown`) → 0020 keeps ATLAS's own frontend out of
the consumer role. 0019 is the concept hub: every later consumer is defined
as "an ADR-0019-disciplined consumer."

**4. Treasury / IntentSpec (0021–0022).** 0021 made the envelope polymorphic
(`ArtifactPayload = Rules | IntentSpec`, canon-5) so a non-rule artifact kind
exists at all; 0022 fixed the attestation policy that 0021's new kind exposed
(kind-selected R7 co-attestation — without it no IntentSpec could ever
verify). Together they are what lets the treasury payment loop
(`treasury-intent-controller` + COMPASS Stage C(b)) consume a *signed*
IntentSpec.

**5. Derived views (0023).** The graph exporter cites into three clusters at
once — 0005 (it recomputes the T4 conflict report), 0016/0019/0020 (it is the
**third** ADR-0019-disciplined consumer, after COMPASS's WASM verifier and
the tic treasury resolver's `ke-artifact-py` fold), and 0021/0022 (IntentSpec
is a first-class node family; `ATTESTED_BY` edges are kind-dependent). It
touches the substrate not at all: no canon change, no gate — the deliberate
mirror-image of 0021's blast radius.

Reading order for a newcomer: 0002 → 0016 → 0019 → 0021 → 0023 — codec, then
verify surface, then trust discipline, then payload polymorphism, then the
first derived view. The lone still-open thread: 0005's Proposed
shared-scenario amendment. (0024 Accepted with its 2026-07-19 merge, PR #17 —
closing 0015 with it, which had sat Proposed from 2026-06-11 until its
channels either shipped or were formally deferred.)
