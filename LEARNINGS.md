# LEARNINGS — Gate 4 build session (2026-06-10 → 13)

A working record of what we learned building Gate 4 (artifact + attestations +
registry lifecycle + consumer-agnostic verification) with multi-agent workflows.
Process lessons first (they cost the most time), then technical ones. Keep this
honest: it includes the mistakes, not just the wins.

---

## Process / orchestration

- **Verification is the product, not the agents' word.** Every phase ended with
  an independent `integration-runner` that re-ran the real gate from the working
  tree, and then *we re-ran the load-bearing checks again* before reporting.
  This caught real things and prevented false greens. The discipline (re-run the
  official gate; recompute empirical numbers from source) is non-negotiable.

- **A dead agent is not a failed build.** Phase 4a's build agent died on a socket
  error and reported `build: null`, but every file had already landed; the
  downstream gate agent re-derived correctness from the tree and was green. A
  truncated/`null` agent report ⇒ **check the tree before retrying** — retrying a
  completed build risks clobbering verified work.

- **Agents refusing to fabricate is the system working.** When Phase 3b's first
  run executed under (accidental) plan-mode read-only, the gate returned RED
  ("can't turn nothing green") and the docs agent refused to write a log for
  code that didn't exist. That's the honesty contract holding even when the
  harness state was wrong.

- **Plan-mode launch trap.** A `Workflow` launched while plan mode is still
  active spawns **read-only** agents → silent no-op build. Answering an
  `ExitPlanMode` dialog with *text* rejects it and keeps plan mode on. Launch a
  build workflow only after the genuine "you have exited plan mode" signal.

- **Single-crate work → one build agent, not a disjoint-file fan-out.** Fan-out
  with a shared interface contract shines for independent files; a single crate's
  interdependent modules are more reliable built by one agent (no mid-crate
  interface drift), then gated + documented by separate agents.

- **Workflow authoring gotcha.** Agent prompts are backtick template literals —
  an inner backtick (markdown ``code``) closes the literal and the script fails
  to parse. Build prompts with array-join + plain quotes. (Also: no TS
  annotations, no `Date.now()`/`Math.random()` in workflow scripts.)

- **Split big phases.** 3a/3b and 4a/4b splits kept each landing CI-testable and
  reviewable, and isolated the toolchain/distribution risk (bindings, publish)
  from the deterministic core.

- **Doc-each-phase + implemented-vs-deferred boundary.** Every phase updated the
  implementation log + brief and *named what was deferred*. Caught one
  overstatement ("registry + S3 v1 COMPLETE" when S3 was local-FS only) — the
  header was corrected to match the body.

---

## Gate / spec discipline

- **Rescopes need an ADR + sign-off before code.** Pulling WASM verification into
  Gate 4 (Phase 4) was recorded as ADR 0016 first; the plan approval was the
  sign-off. Don't silently move gate scope.

- **The spec's non-goals are load-bearing.** "Port the Temporal service here" was
  declined because spec §2 explicitly lists "Temporal orchestration to Rust" and
  "replace the platform's Temporal worker" as non-goals (verified against the raw
  file — an agent's "§38/§42" citation turned out to be *line* numbers, true).
  The durable registry we needed is a **signed append-only event log** (ADR
  0012), not Temporal.

- **Authority boundary is a correctness property, not a slogan.** The T4
  contradiction detector was flagging 52 "Blocking" conflicts on the clean corpus
  because it treated rules answering *different questions* as contradictory — the
  compiler asserting legal truth. Fixed to require a genuinely shared scenario;
  this is what unblocked `structurally_verified`.

---

## Technical invariants (carry these into every future phase)

- **Zero-then-patch content hash.** `artifact_hash` = BLAKE3 over the envelope
  prefix **with the 32-byte hash slot zeroed**, then patched in. Therefore
  `blake3(final .kew bytes) ≠ artifact_hash` *by construction* — verifiers must
  re-zero the slot first. Pinned by a test that *negatively* asserts the naive
  whole-file check fails, so the design can't be "fixed" by weakening it. ADR
  0012's verify wording carried the naive phrasing in three spots — all corrected.

- **Determinism is platform-dependent unless pinned.** The Gate-1 schema-
  determinism gate was green in CI and RED on a fresh Windows checkout: `git
  autocrlf` rewrote the LF-committed schema to CRLF, no `.gitattributes`, CI ran
  ubuntu-only. Fix: `.gitattributes` `eol=lf` + a `windows-latest` CI leg. Any
  byte-exact/hash check is checkout-environment-sensitive.

- **RNG-free verify path.** ed25519 *verify* is deterministic (RFC 8032); only
  keygen/signing touch RNG. Keeping verification RNG-free means PyO3/WASM
  bindings sidestep the `getrandom 0.3` / windows-gnu `dlltool` breakage
  entirely. Signing lives behind a `test-keys` feature with **loud** key ids
  (`test-fixed-seed-1`, `test-expert-fixed-seed-1`, `test-mock-tsa-1`,
  `test-registry-fixed-seed-1`) so golden signatures can never be mistaken for
  production keys.

- **`cfg(test)` is invisible to bins.** Shared fixed-seed test material used by a
  generator/bin needs a cargo `feature`, not `#[cfg(test)]`.

- **Registry state lives in the event log, not the artifact.** Current state =
  the highest-seq event's `new_state`, over a hash-chained, registry-root-signed
  log. State transitions never mutate artifact bytes (attestations append
  post-envelope; T2/T3 and revocation metadata live in registry sidecars). The
  in-envelope `consistency_block` (§8.1) vs §9-attaches-T2/T3-after-compile
  tension is flagged for a follow-up ADR.

- **Offline consumers must see revocation.** `verify_artifact` is pure and takes
  registry state as *data*; the provenance export embeds `registry_state` +
  `event_head_hash`, so an offline consumer (e.g. COMPASS) refuses a non-Published
  pack *even with valid crypto* and detects staleness against a live head. Proven
  by `rejected_when_revoked` + `stale_event_head` tests.

---

## Open follow-ups (as of Phase 4a)
- Gate 4 **Phase 4b**: PyO3 `ke-artifact-py` wheel + `ke-wasm` wasm-bindgen
  verifier + `@platform/atlas-artifact` npm package + 3-language `contract-test.sh`
  (Rust ≡ Python ≡ WASM). Publish + the COMPASS rewire are credentialed follow-ups.
- ADR follow-up: §8.1-vs-§9 `consistency_block` placement.
- Deferred infra: real S3 backend + Object-Lock/versioning; registry-root HSM
  custody + signed key-directory + rotation; real T2/T3 sidecar evidence;
  runtime revocation enforcement (platform/Gate 6); `ke serve` (Gate 5).
