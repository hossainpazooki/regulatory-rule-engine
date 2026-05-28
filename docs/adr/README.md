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

ADRs likely to land first (post Gate 0):

- 0001 Canonical encoding profile (paired with `docs/canonical-encoding.md`)
- 0002 Registry persistence model — S3 manifest layout (spec § 21 resolved)
- 0003 `ke-artifact-py` S3-backed PEP 503 index layout (spec § 21 resolved)
- 0004 Expert key authority (spec § 21.1)
- 0005 Trusted timestamp authority (spec § 21.5)
- 0006 T2/T3 sidecar deployment (spec § 21.3)
- 0007 Package-manager choice (spec § 21.9) — only if pnpm is later adopted
