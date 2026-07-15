# `ke serve` consumer contract (consumer-facing)

**Status:** the surface the COMPASS consumer integrates against to close the live
verification loop. Source of truth is the code, not this doc: request/response
shapes live in `crates/ke-cli/src/serve/dto.rs`; the in-browser verifier exports
live in `crates/ke-wasm/src/lib.rs`. ADR-0018 fixes the endpoint set and the
SSE-not-WebSocket choice; ADR-0019 fixes the fail-closed trust boundary.

> **Three consumers, one discipline (2026-07).** COMPASS is the consumer of
> *this HTTP/WASM surface*; two more ADR-0019-disciplined consumers verify
> through the same `verify_artifact` fold at the crate level instead: the
> treasury intent resolver (`ke-artifact-py`, in
> `treasury-intent-controller/scorer` — ADR-0021/0022) and the graph exporter
> (`ke graph export`, ADR-0023). The fail-closed rules and the
> `ArtifactProvenance` shape below bind all three; the endpoint table binds
> only HTTP consumers.

> **Authority boundary (hard).** `ke serve` and the WASM package are **read-only,
> non-authoritative, preview/verify-only**. Neither signs, attests, publishes,
> nor mutates registry lifecycle state. `/healthz` says so in its banner. The
> served local-FS registry carries a `NON_AUTHORITATIVE` marker (ADR 0012 §6).

## Two ways to consume

1. **HTTP** — `ke serve` exposes the canonical registry/verify surface over REST
   + SSE. Stored-artifact verification (`/verify` by hash) reads the **canonical**
   registry view (G5-1) and is the path COMPASS uses for published packs.
2. **In-browser WASM** — `@platform/atlas-artifact` (the `ke-wasm` build) runs the
   *same* verifier in the browser from raw `.kew` bytes + caller-supplied registry
   evidence. Use this for offline/zero-trust re-verification. The compile/dry-run
   previews are byte-identical twins of the HTTP handlers (`tests/parity.rs`).

## HTTP endpoints

Run with `--features test-keys` or `/verify` returns **HTTP 500** by design
(it needs a verifying-key directory; production key authority is open — ADR-0009).
Start a seeded server with `scripts/serve-published-registry.sh`.

| Method | Path | Request | Success | Notes |
|--------|------|---------|---------|-------|
| GET | `/healthz` | — | `{ok, surface}` | liveness + non-authoritative banner |
| GET | `/resolve` | `?hash=<64hex>` **or** `?env=<e>&tag=<t>` | `ResolutionRecord` | 404 NotFound / 409 Ambiguous / 400 bad hash |
| POST | `/verify` | `{hash:<64hex>, env?, policy?}` | `VerifyResponse` | **HTTP stays 200 even when rejected** |
| POST | `/compile/preview` | `{source:<yaml>}` | `{rules, report}` | non-authoritative; 422 on compile error |
| POST | `/dry-run` | `{source\|hash, facts}` | `{evaluations}` | exactly one of source/hash; 422 on compile/facts error |
| GET | `/events` | — | `text/event-stream` | read-only SSE; opening `ready` frame + keepalive |

`env` defaults to `"local"`; `policy` is `"strict"` (default) or `"permissive"`.
Error bodies are uniform: `{error:<kind>, detail:<msg>}` (kinds: `not_found`,
`ambiguous`, `bad_hash_hex`, `compile_error`, `facts_error`, `internal`).

### `/verify` response (`VerifyResponse`)

```jsonc
{
  "verdict": "verified" | "rejected",
  "rejection": "<human reason>",          // present only when rejected
  "provenance": { /* canonical ArtifactProvenance — see below */ },
  "registry_state": "Published" | "Deprecated" | "Revoked" | "Unknown"
}
```

A **rejection is a valid 200 answer, not a transport error.** The consumer must
gate on the *body*, not the HTTP status.

### Fail-closed semantics (ADR-0019) — verified live

The consumer treats anything other than `verdict:"verified"` **and**
`registry_state:"Published"` as **blocked**. These are the observed responses
from this build (`scripts/serve-published-registry.sh`, fixed-seed test keys):

- **Published artifact** → `{"verdict":"verified", ..., "registry_state":"Published"}`.
- **Fully attested but NOT published** → `{"verdict":"rejected",
  "rejection":"registry state not Published: Unknown", "registry_state":"Unknown"}`.
  *Valid crypto + complete attestations are still rejected* — the core ADR-0019
  guarantee.
- **Unattested / pre-publish** → `{"verdict":"rejected", "rejection":"attestations:
  R6: required type ... missing ...", "registry_state":"Unknown"}`.
- **Unknown hash** → **HTTP 404** `{error:"not_found", ...}` (cannot verify bytes
  that do not exist).

### `ArtifactProvenance` (in every `/verify` and `read_provenance` result)

Carries `regime_id`, `artifact_hash` (byte array), `artifact_kind`
(ADR-0021 — an `Option`: the manifest's kind on success, `null` on a
decode-failed provenance; the field a consumer discriminates kinds by), the
canon triplet (`ir_schema_version` / `codec_version` /
`canonicalization_version` = `0.5.0` / `postcard-1` / `ke-canon-5`, per
ADR-0021), `signer_key_id`, **`is_test_key`**, the
`attestations[]` (each with `attestation_type`, `signer_key_id`, `is_test_key`,
`tsa_class`, `claimed_time_unix`), `registry_state`, `registry_event_head_hash`,
and `exported_at_unix`. **`is_test_key:true`** is present on every field this
build produces — the consumer must surface "TEST key, not production-trusted"
until production-key authority lands (ADR-0009).

## In-browser verifier (`@platform/atlas-artifact`)

Built from `crates/ke-wasm` (see `docs/publish-atlas-artifact.md`). Exports:

```ts
verify_artifact(kew: Uint8Array, keydir_json: string, context_json: string,
                policy_json: string, registry_json: string,
                exported_at_unix: bigint): string   // JSON VerifyResponse-shaped
read_provenance(kew: Uint8Array, registry_json: string,
                exported_at_unix: bigint): string    // canonical provenance JSON
compile_preview(source: string): string              // non-authoritative preview
dry_run(source: string, facts_json: string): string  // non-authoritative preview
```

`verify_artifact` returns `{"verdict":"verified"|"rejected:<reason>",
"registry_state":..., "content_hash":<hex>, "provenance":{...}}`. It **throws only
on malformed JSON inputs** — a verification failure is a normal `rejected:` verdict,
never a throw. The four JSON inputs are the same the Rust/Python contract legs use:

| input | what it is |
|-------|------------|
| `keydir_json` | trusted verifying keys + roles (`scripts/contract-inputs/keydir.json`) |
| `context_json` | env, `now_unix`, supported policy versions (`.../context.json`) |
| `policy_json` | required attestation types + thresholds (`.../policy.json`) |
| `registry_json` | registry evidence: `status`, `event_head_hash` (`.../registry.json`) |

The browser obtains `registry_json` (the live lifecycle state + event head) from
the HTTP surface above; the WASM verifier folds it in and rejects revoked / stale
/ non-Published packs. The native by-`hash` resolve path is intentionally **not**
bound in WASM (authority boundary) — stored-artifact resolution goes through the
canonical HTTP endpoint.
