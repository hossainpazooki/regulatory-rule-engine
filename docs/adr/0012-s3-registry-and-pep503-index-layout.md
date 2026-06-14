# 0012. S3 registry layout + S3-backed PEP 503 package index layout

**Status:** Accepted (sign-off by Hossain, 2026-06-11)
**Date:** 2026-06-11
**Spec references:** §8.1 (artifact structure), §9 (lifecycle state machine), §14 (cross-repo integration / `ke-artifact-py` packaging), §15 (pinning, rollback, revocation), §16.6 (flat-file `.kew` export), §18 (audit reconstruction), §21 (resolved: persistence = S3)
**Brief references:** `dev/briefs/gate-4-artifact-registry-attestation.md` §2 row 5, §4, §5 (Phase 3 / Phase 4)
**Gate:** 4 (Before-Gate-4 checklist item: "S3 + PEP 503 layouts documented")

> Accepted 2026-06-11 (sign-off by Hossain). The v1 layout below is decided; the
> Object-Lock retention period remains an open retention-owner parameter. No
> LLM/AI code sits anywhere in the registry, signing, or index path; resolution
> and write paths are deterministic.

## Context

Spec §21 already **resolved the persistence model: S3** — content-hash artifact
objects, append-only lifecycle events, S3-hosted manifest/tag objects, and an
S3-backed PEP 503 simple index for `ke-artifact-py`. DynamoDB/Redis secondary
indexes are deferred until S3 manifest operations become a *measured* bottleneck.
What is **not** yet written down is the concrete bucket/key layout and — the load-
bearing gap surfaced by the Gate-4 audit — **how the spec's append-only and
"published artifacts are immutable" guarantees (§9, §15) are actually enforced.**
The spec states them as invariants; an S3 prefix does not enforce them by itself.
Convention ("we only ever append") is defeated by a single mis-scoped
`PutObject`, an over-broad IAM role, or a compromised writer key — exactly the
threats §20 and ADR 0009 (key authority) care about.

Constraints this layout must satisfy:

- **§9 / §15 immutability:** artifact bytes and lifecycle events are write-once.
  Published bytes never change. State transitions *append* signed events; they
  never mutate the artifact.
- **§15 rollback:** rollback moves a *tag/policy pointer* to a prior content
  hash. It mutates a pointer, not artifact bytes, and the move is itself a signed
  registry event.
- **§18 audit:** a production decision must record **registry state at resolution
  time** and be reconstructable from `workflow_id → artifact_hash → bytes → trace
  → spans → attestations → evidence → decision`. The registry is the source of
  the "state at resolution time" field.
- **§14 packaging:** the platform pins an **exact** `ke-artifact-py` version *and*
  hash from a PEP 503 simple index; the platform-repo Gate-4 PR installs through
  this index mechanism, not a local wheel path.
- **CLAUDE.md:** `fixtures/` stays read-only in ordinary sessions; the local-FS
  backend is dev/test only and must never be a path by which a non-local runtime
  trusts an object.

Content hash = BLAKE3 (§8); codec = postcard-1 (ADR 0002); signatures = ed25519
(§8). Throughout, `<hash>` is the lowercase hex BLAKE3 of the artifact's canonical
bytes (the `manifest.artifact_hash`, computed via the zero-then-patch derivation).

## Decision

### 1. Two S3 buckets, one trust model

| Bucket (logical) | Holds | Mutability | Enforcement |
| --- | --- | --- | --- |
| **registry** | artifact objects, lifecycle events, manifests, tag/policy pointers, key directory, `.kew` exports | mixed (see below) | Object Lock + versioning + single-writer policy |
| **pkg-index** | PEP 503 simple index pages + `ke-artifact-py` wheels/sdists | write-once | Object Lock + versioning |

Both buckets have **versioning enabled** and a **bucket policy that denies
`s3:PutObject`/`s3:DeleteObject` to every principal except a single
registry-writer role** (`ke-cli publish/attest/transition` running under that
role). Public/anonymous access is fully blocked. The registry-writer role is the
*only* principal that can write, and even it cannot overwrite Object-Lock'd keys.

### 2. registry bucket — key layout

```
s3://<registry>/
  artifacts/<hash>/artifact.kew        # canonical signed bytes (postcard-1)   [WORM]
  artifacts/<hash>/manifest.json       # decoded §8.1 manifest, for index/query [WORM]
  artifacts/<hash>/schema.json         # JSON Schema emitted for this artifact  [WORM]
  events/<hash>/<seq>-<event>.json     # append-only signed lifecycle events    [WORM]
  tags/<env>/<tag>.json                # MUTABLE pointer -> a <hash>            (versioned, not WORM)
  policies/<env>/<name>.json           # MUTABLE policy pointer -> PolicyBundle hash (versioned, not WORM)
  keys/directory.json                  # MUTABLE signer/key directory (versioned, not WORM; see ADR 0009)
  exports/<hash>.kew                   # §16.6 flat-file DR export = copy of artifact.kew [WORM]
```

**Artifact objects** (`artifacts/<hash>/…`) are **content-addressed and
WORM**. The key *is* the BLAKE3 hash, so the store is self-verifying: a reader
recomputes the content hash and rejects on mismatch — per the zero-then-patch
derivation (Erratum, §5 step 3): extract the envelope prefix from `artifact.kew`,
re-zero the 32-byte `artifact_hash` slot, BLAKE3 the zeroed prefix; never a
naive hash of the raw file bytes. `manifest.json`
and `schema.json` are non-authoritative decodings kept beside the bytes purely so
queries and Pydantic generation (§14) don't have to decode every `.kew`; the
authoritative artifact is always the bytes.

**Lifecycle events** (`events/<hash>/<seq>-<event>.json`) are the §9 state
machine as **append-only signed objects**. `<seq>` is zero-padded monotonic
(`0000`, `0001`, …); `<event>` is one of `structurally_verified`, `ml_checked`,
`expert_attested`, `published`, `deprecated`, `revoked`, `tag_moved`,
`policy_moved`. Each event object carries: the artifact `<hash>`, prior state,
new state, the acting authority's key ID and role, the trusted timestamp (ADR
0010), a back-pointer to `<seq-1>`'s object hash (a hash chain, so a missing or
reordered event is detectable), and an ed25519 signature over the canonical event
body by the authority permitted for that transition (§9 transition rules: only
CI authority → `structurally_verified`; only the ML sidecar → `ml_checked`; only
registry policy → `published`; etc.). The **current state of an artifact is the
new-state of its highest-`<seq>` event**, never a mutable field.

**Tag and policy pointers** (`tags/…`, `policies/…`) are the *only* mutable
objects. A pointer is a tiny JSON object `{ "target_hash": "<hash>",
"moved_by_event": "events/<hash>/<seq>-tag_moved.json" }`. **Rollback = write a
new pointer version whose `target_hash` is a prior content hash, and append the
corresponding signed `tag_moved`/`policy_moved` event.** Artifact bytes are never
touched. Because the bucket is versioned, every historical pointer value is
retained and auditable; because each move emits a signed event, the *why/who/when*
is in the WORM event log, not just in S3 version metadata.

### 3. Enforcing append-only and immutability (the audit's correction)

Convention is insufficient. The layout is enforced as:

- **S3 Object Lock in compliance mode (WORM)** is enabled on `artifacts/*`,
  `events/*`, and `exports/*` keys with a retention period set by the
  retention/legal owner. Compliance mode means **no principal — not even the
  account root — can overwrite or delete a locked object version within
  retention.** This is what makes "published artifacts are immutable" and
  "events are append-only" true under key compromise or a mis-scoped IAM change,
  not merely documented.
- **Bucket versioning** is on everywhere so that even the mutable
  `tags/`/`policies/`/`keys/` objects retain full history; a malicious pointer
  overwrite cannot erase the prior pointer value.
- **Single-writer bucket policy:** only the registry-writer role may `PutObject`;
  all other principals are denied write/delete. The platform consumer has
  **read-only** access.
- **Hash-chained events** make a *gap* detectable even if the WORM layer were
  somehow bypassed: a verifier walking `0000..N` checks each `prev_event_hash`.

Trade-off accepted at sign-off (the retention period itself remains an open
retention-owner parameter): compliance-mode Object Lock is irreversible and
imposes a hard minimum retention. The reversible alternative (governance mode +
an SCP denying the bypass permission) is weaker against a root/account compromise
and is listed under Alternatives.

### 4. pkg-index bucket — PEP 503 simple index + exact pinning

```
s3://<pkg-index>/simple/
  index.html                              # lists projects: <a href="ke-artifact-py/">ke-artifact-py</a>
  ke-artifact-py/
    index.html                            # one <a> per wheel/sdist, with hash + metadata
    ke_artifact_py-<ver>-<tags>.whl       # the wheel                              [WORM]
    ke_artifact_py-<ver>.tar.gz           # sdist (optional)                       [WORM]
```

The project page lists each file as a PEP 503 anchor with the **PEP 658/503
hash fragment** in the href and a `data-dist-info-metadata` attribute:

```html
<a href="ke_artifact_py-0.3.0-cp311-...whl#sha256=<sha256>"
   data-dist-info-metadata="sha256=<meta-sha256>">ke_artifact_py-0.3.0-...whl</a>
```

Wheels and the index pages are **WORM** (Object Lock) so a published version's
bytes and its advertised hash cannot change — a republish under the same version
is rejected by the lock, forcing a version bump (matches §15 "published artifacts
are immutable" for the package surface too). The platform pins **exact version +
sha256** in its lockfile; `pip --require-hashes` then fails closed if the index
ever serves different bytes. The wheel `<ver>` corresponds 1:1 to the
`ke-artifact` release that emitted the JSON Schema, so schema, wheel, and golden
fixtures move together (§14 schema-drift prevention).

### 5. Resolution: selector → content hash, and what §18 records

New-workflow resolution (§15) takes a selector `(env, tag | regime_id +
effective_date)` and returns a **content hash**, never a mutable handle:

1. **By tag:** read `tags/<env>/<tag>.json` → `target_hash`.
2. **By regime + effective date:** list `artifacts/*/manifest.json` filtered by
   `regime_id` and `[effective_from, effective_to)` (ADR 0007 closed-open
   semantics; `jurisdiction_time_zone = None` honored exactly, ADR 0007/0008),
   intersected with `published` artifacts for `<env>` (per their event logs),
   pick the unique applicable hash; ambiguity is an error, not a silent choice.
3. **Verify** the resolved object: ~~recompute BLAKE3 over `artifact.kew` and
   confirm it equals `<hash>`~~; read the event log to confirm current state is
   `published` (or the explicitly requested state) for `<env>`.
   **Erratum (2026-06-12):** the struck wording is wrong *by construction* under
   the zero-then-patch derivation (`artifact_hash` is computed over the envelope
   prefix with the 32-byte `artifact_hash` slot zeroed, then patched in), so
   BLAKE3 over the raw `artifact.kew` bytes never equals `<hash>` for a valid
   artifact. The correct verification: extract the envelope prefix from
   `artifact.kew`, re-zero the 32-byte `artifact_hash` slot within that prefix,
   recompute BLAKE3 over the zeroed prefix, and confirm it equals `<hash>`.
   The naive whole-file check would reject every valid artifact.

The resolver then **records, for the §18 audit, at resolution time**:

- `artifact_hash` (the resolved content hash)
- **registry state at resolution time** = the new-state of the highest-`<seq>`
  event object, plus that event's object key (so the exact event is citable)
- the resolving selector (`env`, `tag` or `regime_id`+`effective_date`)
- the tag/policy pointer **S3 version-id** that was read (pins which pointer value
  was in effect even after a later rollback)
- the active `attestation_policy_version` and the attestation IDs/event keys that
  put the artifact in `expert_attested`
- the trusted-timestamp of the `published` event and the resolution timestamp

These feed the §18 runtime audit event ("registry state at resolution time",
"attestation IDs", "attestation policy version") and make the §18 reconstruction
path replayable: the recorded pointer version-id + event key reconstruct *exactly*
what the registry said when the workflow pinned its hash, even if tags later moved.

Temporal pinning (§15) is unchanged by this ADR: resolution runs in a **startup
activity** (non-deterministic), the resolved hash enters workflow history, and
downstream activities load `artifacts/<hash>/artifact.kew` by hash only.

### 6. Local-filesystem backend (dev/test only)

A filesystem backend mirrors the key layout under a root dir
(`artifacts/<hash>/…`, `events/<hash>/…`, `tags/…`, `simple/…`) so `ke-cli` and
the contract test run without S3. It **does not** provide WORM/Object-Lock
guarantees. Therefore:

- The local backend is selectable only via an explicit dev flag/env
  (`KE_REGISTRY_BACKEND=local`); production code paths default to S3.
- Objects produced by the local backend are marked non-authoritative and **a
  non-local runtime policy rejects them** (same posture as the mock TSA in ADR
  0010). It is a convenience for iteration and golden-fixture generation, never a
  publication path.

## Consequences

**Desirable**

- "Published is immutable" and "events are append-only" become **enforced
  invariants** (WORM + versioning + single-writer), not documentation — directly
  closing the audit's flagged gap and hardening against the §20 key-compromise
  risk.
- Content-addressed keys make the store self-verifying; a corrupted or swapped
  byte is caught by recomputing the content hash against the key (via the
  zero-then-patch re-zero procedure — Erratum, §5 step 3).
- Rollback is a pointer move with a signed event and retained pointer history —
  satisfies §15 with zero byte mutation and full auditability.
- §18 "registry state at resolution time" is concretely sourced (highest-seq
  event + pointer version-id), so audits are replayable across later rollbacks.
- Exact version+sha256 pinning + WORM wheels give the platform a hash-locked
  dependency that fails closed on drift (§14).
- No DB to operate in v1; S3 + bucket policy is the whole control plane.

**Undesirable / costs**

- Compliance-mode Object Lock is **irreversible** and forces a retention horizon;
  mistakes (a bad published artifact) can only be *superseded/deprecated*, never
  deleted within retention — which is the point, but needs retention-owner
  sign-off.
- Resolution reads several objects (pointer → manifest(s) → events) and relies on
  S3 read-after-write consistency; under high tag churn a stale pointer read is
  possible. Acceptable in v1 (tags move rarely, only via signed events); a
  DynamoDB index is the documented escape hatch when measured.
- `regime + effective_date` resolution does a `LIST`+filter over manifests —
  fine at corpus scale (tens of rules), the first thing to index later.
- Two buckets + lock config + single-writer policy is real IaC surface that must
  be reviewed as part of the security sign-off.

## Alternatives considered

- **Convention-only append-only (rejected):** "we only ever PutObject new keys."
  Defeated by one mis-scoped write, an over-broad role, or a compromised writer
  key. Fails the exact threat model §20/ADR 0009 target; this is the status quo
  the audit flagged.
- **Governance-mode Object Lock + SCP (alternative, weaker):** reversible by a
  privileged principal, so it does not survive a root/account compromise. Kept as
  a fallback if the retention owner cannot accept compliance mode's
  irreversibility.
- **DynamoDB/Redis as the lifecycle store (deferred per §21):** strong
  conditional-write append semantics and fast resolution, but adds an operational
  datastore and a second source of truth before S3 is shown to be a bottleneck.
  Deferred until measured, exactly as the spec resolves.
- **Mutable "latest" object per regime instead of tags (rejected):** an in-place
  mutable pointer with no event trail loses the §15 "tags move only through
  signed events" guarantee and the §18 replayability of which value was in effect.
- **devpi / CodeArtifact for the package index (rejected for v1):** more features
  but another service to run/authorize; a static S3 PEP 503 index with WORM wheels
  meets §14's exact-pin + same-mechanism-as-staging requirement with far less
  surface. Revisit only if dynamic index features are needed.
- **Embedding the wheel hash only in the lockfile, not the index href
  (rejected):** loses index-side integrity advertisement; PEP 503/658 hash
  fragments let `pip --require-hashes` fail closed without trusting a side
  channel.