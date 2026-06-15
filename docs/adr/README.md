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
- [0005 T4 conflict classes and severities for Gate 2](0005-t4-conflict-classes-gate-2.md) — spec § 12 — **Accepted**
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

Anticipated (later gates — numbers assigned when authored):

- Package-manager choice (spec § 21.9) — only if pnpm is later adopted
- Frontend visual-regression tooling (spec § 21.8) — assigned at the start of 5d
