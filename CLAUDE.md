# ke-workbench — session discipline + repo invariants

This file is loaded into every Claude Code session that operates on this repo.
Treat the rules in this file as **hard constraints**, not preferences. They
exist because the migration plan in
`docs/spec/ke-workbench-rust-migration-spec-v3.1.md` depends on them.

If something here conflicts with the spec, the **spec wins** — open an ADR
in `docs/adr/` to reconcile.

> Global working rules (file-op style, git default, verification, workflows,
> shared agents) are loaded from `~/.claude/`. This file adds the repo's
> spec-driven hard constraints on top, and **overrides the global git default**
> with stricter gate discipline (see Git discipline below). When this file and
> the global rules disagree, this file wins; when this file and the spec
> disagree, the spec wins.

---

## What this repo is

`ke-workbench` is one product with multiple surfaces:

- a Rust workspace under `crates/` (compiler, IR, runtime, artifact, CLI, WASM)
- a React/Vite/TS frontend under `frontend/`
- a YAML rule corpus under `fixtures/rules/` snapshotted from
  `institutional-defi-platform-api/src/rules/data/` via `scripts/bootstrap.sh`
- a registry of signed, content-addressed artifacts (Gate 4 onwards)

The institutional DeFi platform (sibling repo) is the **consumer**. It
executes Rust-compiled artifacts in Python `RuleRuntime` after verifying
hash, signatures, registry state, and typed expert attestations. The platform
does not link the compiler.

Authoritative spec: `docs/spec/ke-workbench-rust-migration-spec-v3.1.md`.

---

## Hard invariants

### Git discipline

- **No commits or pushes from Claude Code.** Hossain owns git history.
- `git mv` is allowed for file moves that preserve history.
- Gate work happens on per-gate branches named `migration/gate-N-*`.
  Hossain merges each gate manually after review.
- Gate boundaries are commit boundaries. **No gate may begin until the prior
  gate's acceptance criteria (spec § 19) are green.**

### File ops

- **Plan Mode is required for design or architectural changes that touch
  ≥ 2 files** (new modules, schema changes, refactors, gate-scope decisions).
  Mechanical multi-file edits that must move together — version bumps, pin
  updates, dependency renames, CI-failure fixes, doc cross-link updates —
  may proceed without Plan Mode.

### `fixtures/` is read-only inside ordinary sessions

- Never hand-edit anything under `fixtures/`.
- Updates flow only through documented sync/generation scripts:
  - `scripts/bootstrap.sh` — refreshes `fixtures/rules/` from the platform
    repo and rewrites `fixtures/rules/SOURCE.md` with the platform commit SHA.
  - Gate 1+ adds fixture-generation tooling that regenerates dependent
    fixtures atomically.
- The platform commit SHA recorded in `fixtures/rules/SOURCE.md` is the
  reference point. `scripts/differential-test.sh` and
  `scripts/equivalence-harness.sh` must verify the platform checkout still
  points at this exact commit before running, or the gate run is invalid.

### Frontend preservation through Gate 4

- Through Gate 4, frontend pages, routes, and public component contracts are
  preserved. The frontend continues to consume the external backend via
  `VITE_API_URL`.
- Frontend feature work unrelated to the migration is allowed on short-lived
  branches rebased onto the latest completed gate. "Preserved" means
  behavior-preserved, not directory-frozen.
- Gate 5 rewires page-by-page behind `VITE_USE_LOCAL_KE_API`,
  `VITE_USE_WASM_PREVIEW`, `VITE_USE_REVIEW_UI` flags (spec § 7.4).

### Authority boundaries (spec § 5, § 10, § 13)

- **Compiler authority:** structural validity only. Never legal truth.
- **AI authority:** may propose edits, rationales, source-span mappings,
  scenario candidates, conflict explanations. **May not attest, publish,
  revoke, or silently modify committed rules.**
- **Domain expert authority:** the only authority that can sign typed
  attestations bound to a specific artifact hash.
- **Registry authority:** the only authority that can transition artifact
  lifecycle state after verifying signatures, keys, revocation, and required
  checks.

### WASM is preview-only (spec § 6, § 16)

- Browser code may not sign, attest, publish, or otherwise produce
  authoritative artifacts. WASM compile/dry-run output is non-authoritative.
- The canonical compile path is `ke-cli compile` against an authoritative
  registry. Differences between WASM preview and canonical compile must be
  surfaced, never silently published.

---

## Session conventions

### Per-batch verification

After each meaningful batch of changes:

```bash
cargo test --workspace          # once crates have content (Gate 1+)
cd frontend && npm test         # once Gate 0 lands
```

If either fails, fix the regression before continuing. Don't paper over with
`--no-verify`, `-Awarnings`, or skipped tests.

### Platform repo access (spec § 4.5)

Several gates depend on the sibling `institutional-defi-platform-api`
checkout. Resolution order:

1. `${PLATFORM_REPO}` environment variable, if set.
2. `../institutional-defi-platform-api` (sibling default).

All scripts that need the platform repo must:

- fail fast if the checkout is missing
- fail fast if relevant files are dirty (e.g. `bootstrap.sh` rejects any
  modification under `src/rules/data/`)
- record the platform commit SHA in their output

### Shell

Scripts under `scripts/` use bash with a `#!/usr/bin/env bash` shebang. On
Windows they run under Git Bash. PowerShell is the user's primary shell but
scripts target POSIX bash for portability with CI. Do not duplicate scripts
in PowerShell unless an ADR justifies it.

---

## Open decisions (spec § 21)

These need answers before specific gates begin. Don't proceed past the gate
that depends on them:

| Decision | Gate gated by |
|----------|---------------|
| Expert key authority | Gate 4 |
| T2/T3 production policy | Gate 4 |
| T2/T3 sidecar deployment | Gate 4 |
| Legal source text storage | document source coverage (Gate 2+ if promoted) |
| Trusted timestamp authority | Gate 4 |
| Revocation behavior | Gate 6 |
| Review UI follow-up scope | Gate 5 (minimum scope is unblocked) |
| Frontend visual regression tooling | Gate 5 polish |
| Package-manager migration (pnpm) | not blocking — needs ADR if pursued |

Resolved in v3.1: registry persistence (S3-backed v1) and `ke-artifact-py`
package index (S3-backed PEP 503 simple index).

---

## When you're stuck or unsure

- Read the relevant spec section first. Section pointers are in the spec's
  reading order at the top of v3.1.
- If the spec is ambiguous, write an ADR in `docs/adr/` proposing the
  resolution rather than guessing in code.
- If a gate's acceptance criteria can't be met, **stop**. Don't lower the
  bar — surface the blocker to Hossain.
