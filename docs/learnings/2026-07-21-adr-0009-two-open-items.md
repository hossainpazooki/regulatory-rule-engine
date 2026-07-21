ts: 2026-07-21T14:09:43Z
commit: 151036a (branch docs/adr-0024-acceptance-stamp)
session: 7f20dfba-7a07-4c11-a7e7-5be8c9e7d0af
status: verified

fact: "Production key authority is open" overstates the gap. ADR-0009 is
Accepted with the v1 key architecture decided (directory-resolved ed25519
identities, mandatory expiry, directory-carried revocation, KMS/HSM registry
root per its § 3); exactly TWO operational items survived acceptance as
open — the IdP-backed vs self-managed custody pick (§ 1) and the
retroactive-vs-prospective compromise scope (§ 5). Any key-authority ADR
(0025 drafted this session, PR #19) should close those two, not re-decide
the architecture.

basis: `sed -n '29p' docs/adr/0009-expert-key-authority-and-revocation.md`
(2026-07-21T14:09:43Z @ 151036a) → "> Accepted 2026-06-11 (sign-off by
Hossain); the choices below are the decided v1. Two operational items remain
open after acceptance: the IdP-backed vs self-managed pick (§ 1) and the
retroactive-vs-prospective compromise scope (§ 5)."

re-verify: sed -n '29p' docs/adr/0009-expert-key-authority-and-revocation.md
