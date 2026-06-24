# SEED PROMPT — COMPASS live ATLAS verification (prerequisites now satisfied)

> **What this is.** A copy-paste seed for a **COMPASS session** to close the live
> verification loop. Unlike the earlier seed, the ATLAS-side prerequisites are
> **done and merged** (PR #10 on `origin/main`): the browser verifier builds, a
> Published registry can be served, and the consumer contract is documented. This
> session integrates against them. Parked in ATLAS `dev/briefs/` so the contract
> and the consumer task live side by side; it implies **no ATLAS change**.

---

You are operating in the **COMPASS** repo (`cross-border-compliance-navigator`,
sibling of `regulatory-rule-engine`/ATLAS). COMPASS is the **verify-only,
in-browser consumer** of ATLAS artifacts — it does not sign, publish, attest, or
execute the rule engine. Read COMPASS's own `CLAUDE.md` (esp. its git rule and
the "ATLAS provenance" section) and, in the ATLAS repo,
`docs/consumer-serve-contract.md` + ADR-0019 (the fail-closed trust boundary)
before touching code.

## Confirm starting state (don't assume — verify)
- `git branch --show-current` / `git log --oneline -1` — last known: branch
  `feat/next15-phase-c1`, a "live-session org redesign + Desk MVP + ATLAS
  provenance consumer" commit. Confirm and note drift.
- Today COMPASS surfaces provenance from a **static snapshot**
  (`src/shared/config/atlas-provenance.json`), with the rewire seam already in
  place: `src/shared/atlas/provenance.ts` defines `WASM_VERIFY_ENABLED`
  (reads `NEXT_PUBLIC_USE_WASM_VERIFY`, default **false**, no runtime path), and
  `@platform/atlas-artifact` is **not yet a dependency**. `DeskHome` /
  `RegimeProvenanceCard` render the snapshot with honest "not re-verified / TEST
  key / Published requires live registry" disclosures.

## Prerequisites — NOW SATISFIED (here is how to obtain each)
1. **Browser verifier** `@platform/atlas-artifact` — built and publish-ready, but
   `npm publish` is gated on an open scope decision
   (ATLAS `dev/briefs/npm-publish-atlas-artifact-decision.md`). **Don't wait on
   it** — consume the real build via a `file:` dependency:
   ```jsonc
   // COMPASS package.json
   "dependencies": {
     "@platform/atlas-artifact": "file:../regulatory-rule-engine/crates/ke-wasm"
   }
   ```
   First build the bindings once in the ATLAS checkout (see
   `docs/publish-atlas-artifact.md` §1: `cargo build -p ke-wasm --target
   wasm32-unknown-unknown --release` + `wasm-bindgen --target bundler --out-dir
   crates/ke-wasm/pkg`). Swap `file:` → the registry version once published.
2. **Running registry** with Published artifacts — run, in the ATLAS checkout,
   `scripts/serve-published-registry.sh` (prints the published hash + a verify
   curl; serves on `127.0.0.1:9999` by default). Built `--features test-keys`.
3. **Contract** — `docs/consumer-serve-contract.md` documents every endpoint
   shape, the `verify_artifact(...)` signature, the four verifier inputs
   (`scripts/contract-inputs/{keydir,context,policy,registry}.json`), and the
   live-observed fail-closed responses.

## Goal
Move COMPASS from snapshot-render to **live verification with ADR-0019 fail-closed
semantics**: a pack is "trusted" only when `verdict == "verified"` **and**
`registry_state == "Published"`; `Unknown` / `Revoked` / `Deprecated` / a rejected
verdict / a 404 are all **blocked**, surfaced with the specific reason.

## Choose the verification path (then say which, and why)
- **HTTP path (simplest):** `POST /verify {hash}` to `ke serve` — the server
  verifies against the canonical registry (G5-1) and returns
  `{verdict, registry_state, provenance}`. COMPASS gates on the body.
- **In-browser WASM path (zero-trust, ADR-0019 "re-derive trust"):** call
  `verify_artifact(kew, keydir_json, context_json, policy_json, registry_json,
  exported_at_unix)` in the browser. **Caveat:** this needs the raw `.kew` bytes
  *and* live `registry_json` (lifecycle state + event head). `ke serve` exposes
  registry evidence via `/resolve`, but has **no raw-bytes endpoint** — decide how
  COMPASS obtains `.kew` bytes (bundle the `.kew` export per G5-4, or add a bytes
  channel). If bytes aren't readily available, start with the HTTP path and layer
  WASM zero-trust verification second.

## Implementation
1. Add the dependency (Step-1 `file:` bridge); gate everything on
   `WASM_VERIFY_ENABLED` / `NEXT_PUBLIC_USE_WASM_VERIFY` and a new
   `NEXT_PUBLIC_ATLAS_REGISTRY_URL` (add to `.env.example`; build-time assert it's
   set when verify is on).
2. Implement the chosen verification call; map the result to a typed model — do
   **not** invent fields the verifier doesn't return.
3. **Fail-closed gate (ADR-0019):** reject unless `verified` AND `Published`;
   `Unknown`/registry-unreachable/timeout ⇒ **blocked**, never "render anyway".
4. Verify typed attestations: check `signer_role` against the required-role policy
   (SourceFidelity / ScenarioCoverage / PublicationApproval) and TSA time claims;
   flag unverified attestations rather than hiding them.
5. UI in `RegimeProvenanceCard` / `DeskHome`: ✅ "Verified — Published" / ❌
   "Revoked"/"Rejected: <reason>" / ⚠️ "Unknown (registry unavailable — blocked)" /
   "Verifying…". Replace the static disclosure with live status; **keep the
   `is_test_key:true` "TEST key — not production-trusted" disclosure** until ATLAS
   ships production keys (ADR-0009 open).

## Constraints
- **Consumer-only authority:** never sign/attest/publish/mutate from the browser;
  WASM output is non-authoritative.
- **Git:** follow COMPASS's `CLAUDE.md` git rule (default: Hossain owns history —
  output commands, don't commit; only flip if that file says web-driven). Risky
  work on a branch off `feat/next15-phase-c1`.
- **Scope:** COMPASS repo only. If you hit an ATLAS bug, write it up — don't patch
  across the boundary.
- **Out of scope:** prod-key rotation in the snapshot, verification-failure audit
  logging, perf caching — note as follow-ons.

## Verification gate (evidence, not assertion)
Before claiming done, RUN and paste output for: `npm test` (green + new
fail-closed/blocked-state tests — assert a non-Published hash is blocked **even
with valid attestations**, the core ADR-0019 case), `npm run lint`, `next build`.
Demonstrate against a live `serve-published-registry.sh`: a Published hash renders
"Verified — Published"; the seeded `Unknown` hash renders blocked.

First turn deliverable: confirmed starting state + chosen verification path (HTTP
vs WASM, with the bytes-availability reasoning) — a short plan, not code.
