# Brief — publishing `@platform/atlas-artifact`: state + the one decision to make

> **What this is.** A move-forward brief for **point 2** of the 2026-06-23 live-
> verifier unblock: getting the browser verifier package off this machine and into
> COMPASS's hands. The package is **built and publish-ready**; one publish-target
> decision is the only thing standing between "real verifier available locally" and
> "real verifier installed from a registry." Full mechanics live in
> `docs/publish-atlas-artifact.md` — this brief is the decision + the path, not a
> re-derivation.

## Where we are (done, in-repo, verified)

- `crates/ke-wasm/package.json` is publish-ready: `@platform/atlas-artifact@0.1.0`,
  `private:false`, `publishConfig.registry = https://npm.pkg.github.com`,
  `repository` set, and `files` now includes the bundler's `pkg/ke_wasm_bg.js`
  (it was missing — the package would have published broken).
- The browser bindings build clean on this windows-gnu box (RNG-free wasm32 path):
  `cargo build -p ke-wasm --target wasm32-unknown-unknown --release` +
  `wasm-bindgen --target bundler --out-dir crates/ke-wasm/pkg`.
- `npm pack --dry-run` (run from `crates/ke-wasm/`) → 6 files, ~380 kB, all four
  exports present. **No actual publish has happened** (outward-facing + credentials
  = Hossain's call).
- Verifier signs/verifies with **fixed-seed TEST keys** (`is_test_key:true`).
  Publishing makes it *available and real*, not *production-trusted* — production
  key authority is still the open ADR-0009 decision. Keep that disclosure in any
  consumer UI.

## The one blocker — GitHub Packages scope ownership

GitHub Packages keys an npm package to the **owner** named in its scope. The
package scope is `@platform`, but the repo owner is `hossainpazooki` and there is
**no `platform` GitHub org**. So `npm publish` to `npm.pkg.github.com` will be
rejected as-is. This is a naming/ownership decision, **not** a build problem — the
built package is correct under every option below.

## Decision — pick one path

| # | Path | Keeps `@platform/atlas-artifact` import? | Cost / when |
|---|------|------------------------------------------|-------------|
| **A** | Create/own a **`platform` GitHub org**, publish there | ✅ yes | one-time org setup; best if `@platform/contracts`,`@platform/engine` will also be published. **Recommended** if the `@platform/*` family is real. |
| **B** | **Private registry** (Verdaccio / npm org) keeping `@platform` | ✅ yes | stand up/host a registry; drop the GitHub `publishConfig`. |
| **C** | **Rename scope** → `@hossainpazooki/atlas-artifact` for GitHub Packages | ❌ no — update COMPASS import + all `@platform/atlas-artifact` refs | quickest publish; most downstream churn. Avoid unless A/B rejected. |

A and B preserve the contract COMPASS already imports; C trades that for the
fastest publish. My recommendation: **A** if the `@platform/*` package family is
intended to be real and shared; otherwise **B** to keep the name without org
overhead.

## You can move forward NOW without deciding — the `file:` bridge

COMPASS does **not** need to wait for the publish. Point it at the local build and
integrate the real verifier today:

```jsonc
// COMPASS package.json (separate repo — not an ATLAS change)
"dependencies": {
  "@platform/atlas-artifact": "file:../regulatory-rule-engine/crates/ke-wasm"
}
```

Run `scripts/serve-published-registry.sh` for the HTTP registry/verify surface the
browser verifier folds in (`docs/consumer-serve-contract.md`). Swap `file:` →
the published version once a path above is chosen. This is the unblock for the
COMPASS consumer session; the registry decision can land in parallel.

## Publish commands once the path is chosen (Hossain-run)

```bash
# .npmrc (NOT committed) — auth token with write:packages
//npm.pkg.github.com/:_authToken=${GITHUB_TOKEN}
@platform:registry=https://npm.pkg.github.com   # (or @hossainpazooki under path C)

# build pkg/ first (see docs/publish-atlas-artifact.md §1), then:
cd crates/ke-wasm && npm pack --dry-run && npm publish
```

For **path C**, first change `name` in `crates/ke-wasm/package.json` to
`@hossainpazooki/atlas-artifact` and update the COMPASS import.

## Pointers

- Full runbook + build steps: `docs/publish-atlas-artifact.md`
- Consumer-facing surface (what COMPASS builds against):
  `docs/consumer-serve-contract.md`
- Live-verifier unblock record + evidence: `docs/gate-5-implementation-log.md`
  (§ "Live verifier unblock (2026-06-23)")
