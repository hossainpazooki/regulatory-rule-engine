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
