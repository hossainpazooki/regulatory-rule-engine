# Canonical encoding profile

Placeholder. Authored in Gate 1.

This document will specify the deterministic encoding rules that `ke-artifact`
applies before content-addressing artifacts with BLAKE3. It covers:

- field ordering and map key ordering
- optional-field representation
- numeric representation
- string normalization
- schema versioning (`ir_schema_version`)
- codec versioning (`codec_version`)
- canonicalization versioning (`canonicalization_version`)
- JSON Schema emission rules (field ordering, enum representation,
  `$defs` ordering, reference naming, metadata) — see spec § 8

Until this is written, the IR types in `crates/ke-core/src/ir/` are not
considered frozen and golden fixtures under `fixtures/artifacts/` may not be
generated.

See spec § 8 (artifact contract), § 8.3 (golden test vectors), § 8.4
(effective dates and jurisdiction time).
