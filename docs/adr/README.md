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

Anticipated (later gates — numbers assigned when authored):

- Registry persistence model — S3 manifest layout (spec § 21 resolved) — Gate 4
- `ke-artifact-py` S3-backed PEP 503 index layout (spec § 21 resolved) — Gate 4
- Expert key authority (spec § 21.1) — Gate 4
- Trusted timestamp authority (spec § 21.5) — Gate 4
- T2/T3 sidecar deployment (spec § 21.3) — Gate 4
- Package-manager choice (spec § 21.9) — only if pnpm is later adopted
