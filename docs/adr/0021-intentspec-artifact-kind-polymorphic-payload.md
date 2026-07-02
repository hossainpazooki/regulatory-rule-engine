# 0021. IntentSpec artifact kind — polymorphic envelope payload for non-rule artifacts

**Status:** Proposed (draft for review)
**Date:** 2026-07-01
**Spec references:** § 8 (artifact contract), § 8.1 (structure), § 8.2 (kinds), § 8.3 (golden vectors), § 14 (consumer surface / bindings)
**Amends:** § 8.2 — adds a fifth artifact kind (`IntentSpec`) to the four-kind list, which does not currently name it.
**Related ADRs:** 0002 (postcard codec — `canonicalization_version` semantics), 0003 (no floats in IR), 0016 (consumer-agnostic verify + `ArtifactProvenance` + 3-language contract), 0009 (expert key authority), 0012 (S3 registry / PEP 503).
**Gate:** proposes a new canonicalization-boundary migration gate (number TBD by Hossain). It is **not** a same-gate addition — it re-pins every golden.

## Context

An external program — the treasury intent-gated action loop (a separate design,
its own repos) — needs ATLAS to author, verify, sign, and register a new artifact
type, **IntentSpec**: per action class, the declared authorization criteria
(name + threshold + `stable|volatile` tag), an idempotency key definition, and the
source spans they derive from. It must flow through the identical
T0–T4 + attestation + registry lifecycle as rule artifacts, and be consumed by
hash. Only ATLAS can supply the property the loop is built on: *what authorizes a
payment — including what counts as a duplicate — is a signed, expert-attested,
registry-governed artifact*, not adapter-local config.

The blocker is structural, and the repo already documents it:

1. **The envelope is RuleIR-oriented.** `Artifact.compiled_ir: Vec<RuleIR>`
   (`crates/ke-artifact/src/artifact.rs:84`) is the **only** payload, and it sits
   inside the hashed + signed envelope. `crates/ke-artifact/src/bin/gen-golden-artifacts.rs`
   states it plainly: *"The Phase-1 `Artifact` envelope is RuleIR-oriented
   (`compiled_ir: Vec<RuleIR>`); a `PolicyBundle` has no representation in it"* —
   which is why its golden is skipped. Three of the four declared kinds
   (`EquivalenceMatrix`, `TestCorpus`, `PolicyBundle`) have **no payload
   representation today**; only `RegimePack` is built. IntentSpec is the same
   situation. Its body is definitively **not** `Vec<RuleIR>`.
2. **Nothing branches on `artifact_kind`** in the verify / attestation / publish /
   resolve path. `verify_artifact`, `verify_attestation_set`, `can_transition`,
   `resolve`, and `publish.rs` never read it (it is used only for hash-offset math,
   SQL rendering, and the compile path, which hardcodes `ArtifactKind::RegimePack`).
   A new kind is therefore **verified and published exactly like a RegimePack** —
   an IntentSpec carrying a rule payload would validate with no error.
3. **Attestation policy is kind-agnostic.** `AttestationType` is a closed 5-variant
   enum with no treasury/intent type; `default_verification_policy()` is a single
   kind-independent constant. There is no way today to *require* a treasury-specific
   expert attestation for an IntentSpec.
4. **IntentSpec is absent from the spec** (§ 8.2 lists four kinds; a repo-wide grep
   for `IntentSpec` returns nothing) — so this is a spec amendment plus code.
5. **Consumer surface (§ 14, ADR-0016).** The 3-language contract test
   (`scripts/contract-test.sh`) gates only the folded `verify_artifact` output —
   the verdict plus the `ArtifactProvenance` projection. `ArtifactProvenance`
   **omits `artifact_kind`** today. Per-field payload accessors (`iter_rules`,
   `source_span_index`) are Python-only and *not* gated by the contract test —
   which is correct, because the payload has a single consumer.

## Decision

Introduce `IntentSpec` as a first-class artifact kind by making the envelope
payload **polymorphic**, rather than bolting a second field onto a rule-shaped
envelope.

1. **Polymorphic payload.** Replace `compiled_ir: Vec<RuleIR>` with a sum type in
   the envelope — e.g. `payload: ArtifactPayload` where
   `enum ArtifactPayload { Rules(Vec<RuleIR>), IntentSpec(IntentSpecIR) }`. `RuleIR`
   content is unchanged; `Rules(_)` carries exactly what `compiled_ir` carried.
2. **New canonical IR type.** Add `IntentSpecIR` in `ke-core` (subject to the
   canonical profile and ADR-0003 no-floats rule): `action_class`, `criteria:
   [{ name, threshold, volatility: Stable|Volatile }]`, an `idempotency`
   definition (payer-scoped key + scope), and a source-span binding analogous to
   `source_span_index`. Exact field set is an open question below.
3. **Append the enum variant LAST.** `ArtifactKind::IntentSpec` is appended after
   `PolicyBundle`. Reason (per ADR-0002 / postcard): a mid-list insert changes the
   encoded discriminant *value* of later variants, which changes the content-hash
   *value* of existing artifacts and mis-decodes committed goldens. (The
   hash-*offset* is unaffected — it is a constant 1 byte for ≤128 variants — so the
   append rule is about discriminant-value stability, not offset.)
4. **Per-kind payload dispatch.** `decode_artifact` / `validate_manifest` must
   enforce kind ↔ payload agreement: a `RegimePack` must carry `Rules(_)`, an
   `IntentSpec` must carry `IntentSpec(_)`. This closes the current silent failure
   where an IntentSpec with a rule payload validates cleanly.
5. **Kind-aware attestation policy.** The publish gate and `verify_attestation_set`
   select required attestation types **by kind**. IntentSpec requires its own
   attestation set (the "what authorizes a payment" expert sign-off) — either an
   existing `AttestationType` or a new one appended last. This is what makes the
   treasury conviction enforceable rather than kind-blind.
6. **Consumer surface.** Add `artifact_kind` to `ArtifactProvenance` (populated in
   **both** `artifact_provenance()` and `decode_failed_provenance()`), so all three
   legs plus `ke serve` discriminate kinds and the 3-language contract test gates
   it. Criteria extraction for the downstream reader is a **Python payload accessor
   analogous to `iter_rules`** — single-consumer (only the treasury resolver reads
   the payload; WASM/COMPASS only verify), so it is deliberately *not* on the
   contract-test path, consistent with `iter_rules`/`source_span_index`.
7. **Versioning + goldens.** Bump `IR_SCHEMA_VERSION` (new IR type + payload
   variant) and `CANONICALIZATION_VERSION` (the envelope payload encoding changes
   for every artifact — a sum type prepends a discriminant to the payload bytes;
   per ADR-0002 a layout change is a canonicalization bump). `CODEC_VERSION`
   (`postcard-1`) is unchanged. Regenerate **all** golden artifacts atomically via
   `gen-golden-artifacts.rs` (never hand-edit `fixtures/`), add an IntentSpec
   golden, and re-pin `contract-test.sh`. Add `"IntentSpec"` to the hand-maintained
   schema enum (`crates/ke-core/src/schema/defs.rs` `artifact_kind()`) **and** add a
   test asserting the s_enum list equals the `ArtifactKind` variants — closing the
   current silent drift (the list is not compiler-checked against the enum).
8. **Authoring path.** `compile.rs` hardcodes `RegimePack`; add a `--kind` flag or a
   dedicated IntentSpec authoring subcommand (its input is not rule YAML). Fixed-seed
   `test-keys` gating is unchanged; production keys remain an ADR-0009 / HSM concern,
   out of scope.

### Shared trace contract (informative — not built by this ADR)

The downstream loop closes over three hashes: `rule_artifact_hash`,
`intent_spec_hash`, and `intent/trajectory_hash`. The IntentSpec artifact produced
here supplies the second. The authorization gate's append-only `ACHIEVED` log entry
carries `{intent_id, idempotency_key, rule_artifact_hash, intent_spec_hash,
trajectory_hash, seq}`, and the settlement adapter recomputes from it. Recorded
only so the artifact's hash identity stays coherent with its consumer; the gate and
adapter live in other repos and are out of scope here.

## Consequences

- **Desirable.** Implements the payload polymorphism § 8.2 always implied but the
  build deferred — `EquivalenceMatrix`, `TestCorpus`, and `PolicyBundle` each become
  a future `ArtifactPayload` variant, so the one-time canonicalization bump is paid
  once for all non-rule kinds. Kind ↔ payload dispatch closes the "IntentSpec
  validates as rules" silent failure. Kind-aware policy makes per-kind expert
  attestation enforceable. `artifact_kind` on the provenance closes the "consumer
  cannot tell an IntentSpec from a RegimePack" gap.
- **Undesirable / accepted.** This is a **breaking artifact-format change**: every
  existing artifact's bytes and content hash change, all goldens regenerate, the
  3-language contract re-pins, and `CANONICALIZATION_VERSION` bumps — one-time, but
  it touches every fixture. Scope exceeds a normal gate addition, hence its own
  gate. Verify stays pure/RNG-free; the policy dispatch is the only new
  kind-awareness on the verify side.
- **Authority unchanged.** Verify-only bindings stay verify-only (spec § 6); the new
  payload adds no crypto; signing stays `test-keys`/ADR-0009-gated. AI may propose
  an IntentSpec; only a domain expert attests it and only the registry publishes it.

## Alternatives considered

- **Bolt-on optional field** (`intent_ir: Option<IntentSpecIR>` appended after
  `compiled_ir`). Rejected: it re-encodes every artifact anyway (postcard appends
  `0x00` for `None` → byte change → same canonicalization bump and golden
  regeneration as the sum type), yet leaves `compiled_ir: Vec<RuleIR>` semantically
  wrong for a non-rule kind (an IntentSpec would carry an empty rules vec plus a
  populated option — two payload fields, one always empty) and does not generalize
  to the other three non-rule kinds. Same blast radius, worse model.
- **Force IntentSpec into `Vec<RuleIR>`** (encode criteria as degenerate rules).
  Rejected: avoids the canonicalization bump but is semantically false — it discards
  volatility/idempotency/threshold typing, defeats kind ↔ payload dispatch, and is
  exactly the "validates-as-rules" silent failure this ADR closes. The treasury
  design requires criteria as first-class typed content.
- **A separate artifact envelope per kind.** Rejected: duplicates the manifest /
  hash / sign / attest / registry machinery per kind; ADR-0016's whole premise is
  one envelope and one verify surface.
- **Do nothing in ATLAS; pass criteria to the gate as unsigned params.** Rejected:
  it discards the reason IntentSpec is an ATLAS artifact — criteria must be signed,
  expert-attested, and registry-governed by hash.

## Open questions (for review before fixtures freeze)

1. **Gate number and sequencing** — Hossain's call; this is a canonicalization
   gate that re-pins every golden.
2. **Attestation semantics** — does IntentSpec reuse an existing `AttestationType`
   (e.g. `SourceFidelity` + `PublicationApproval`) or require a new treasury/
   authorization type? Needs the treasury attestation semantics pinned first.
3. **`IntentSpecIR` field set** — confirm criteria / threshold / volatility /
   idempotency-key-definition / source-spans against the treasury design before the
   IR shape is frozen (fixtures depend on it).
