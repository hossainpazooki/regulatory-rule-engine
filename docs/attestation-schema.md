# Typed attestation schema

Placeholder. Authored before Gate 4.

This document will pin down the on-disk shape of typed expert attestations
(see spec § 10). It must specify, for each attestation type:

- `source_fidelity`
- `interpretation`
- `scenario_coverage`
- `equivalence_claim`
- `publication_approval`

…the exact set of bound fields, signature scheme (ed25519 + canonical
encoding), trusted-timestamp envelope, key identity and revocation
verification rules, and platform rejection conditions (see spec § 10
"Platform rejection rules").

Open decisions blocking authorship of this document:

- Expert key authority (spec § 21.1)
- Trusted timestamp authority selection (spec § 21.5)
- T2/T3 sidecar deployment shape (spec § 21.3)

**Gate 1 note:** the *enum* shapes this document will bind are already frozen in
`ke-core`: `AttestationType` (the five kinds above), `T2T3Mode`,
`RevocationPolicy`, and the `PolicyBundle` carrier (see
`crates/ke-core/src/manifest.rs` and the JSON Schema at
`crates/ke-core/schema/ir.schema.json`). Gate 1 only froze the *shapes* so
canonical encoding can round-trip them; the binding fields, signature scheme,
and rejection rules remain Gate 4 work gated on the decisions above.
