# Publishing `@platform/atlas-artifact` (the in-browser verifier)

The browser verifier the COMPASS consumer depends on is the `ke-wasm` crate,
packaged as the npm module **`@platform/atlas-artifact`** (`crates/ke-wasm/`).
`pkg/` is a **generated, gitignored** wasm-bindgen artifact — it must be built
before packing/publishing.

> **Who runs this.** The actual `npm publish` is an outward-facing action needing
> credentials — **Hossain runs it**, not Claude Code. Everything up to and
> including `npm pack --dry-run` is reproducible without credentials and has been
> verified in-repo.

## 1. Build the browser bindings

`wasm-bindgen-cli` must match the crate pin **exactly** (`=0.2.95`); `wasm-pack`
is **not** required (we drive `wasm-bindgen` directly, as CI does). The wasm32
build is RNG-free, so it builds cleanly on the windows-gnu toolchain.

```bash
cargo install wasm-bindgen-cli --version 0.2.95 --locked   # once, if missing
cargo build -p ke-wasm --target wasm32-unknown-unknown --release
wasm-bindgen --target bundler \
  --out-dir crates/ke-wasm/pkg --out-name ke_wasm \
  target/wasm32-unknown-unknown/release/ke_wasm.wasm
```

This emits the five `pkg/` files the package ships: `ke_wasm.js`,
**`ke_wasm_bg.js`**, `ke_wasm_bg.wasm`, `ke_wasm.d.ts`, `ke_wasm_bg.wasm.d.ts`.
(`ke_wasm.js` imports `./ke_wasm_bg.js` — both must be present, which is why
`package.json` `files` lists both.) Confirm the four exports:

```bash
grep -E 'export function (verify_artifact|read_provenance|compile_preview|dry_run)' \
  crates/ke-wasm/pkg/ke_wasm.d.ts        # expect 4 matches
```

## 2. Inspect the tarball (no publish)

```bash
cd crates/ke-wasm && npm pack --dry-run
```

Expected: 6 files (`package.json` + the 5 `pkg/` files), name
`@platform/atlas-artifact@0.1.0`, ~380 kB packed. Bump `version` on each release.

## 3. Publish to GitHub Packages

`publishConfig.registry` is already set to `https://npm.pkg.github.com`. Auth via
a token with `write:packages` (never commit it):

```bash
# ~/.npmrc (or a repo-local .npmrc you do NOT commit)
//npm.pkg.github.com/:_authToken=${GITHUB_TOKEN}
@platform:registry=https://npm.pkg.github.com
```

```bash
cd crates/ke-wasm && npm publish     # after step 1 has produced pkg/
```

> ⚠️ **GitHub Packages scope constraint — decide before first publish.** GitHub
> Packages keys a package to the **owner** (user/org) in its scope. The package
> is named `@platform/atlas-artifact`, but the repo owner is `hossainpazooki`, and
> there is (as of writing) no `platform` GitHub org. Options, in order of least
> disruption to the COMPASS import (`import ... from '@platform/atlas-artifact'`):
> 1. **Create/own a `platform` GitHub org** and host the package there — keeps the
>    name COMPASS already imports. Preferred if the `@platform/*` family
>    (`@platform/contracts`, `@platform/engine`) will also be published.
> 2. **Use a private registry** (Verdaccio / npm org) that doesn't impose
>    scope==owner — keeps `@platform`, drop the GitHub `publishConfig`.
> 3. **Rename the scope** to `@hossainpazooki/atlas-artifact` for GitHub Packages
>    — simplest to publish, but requires updating the COMPASS import + any
>    `@platform/atlas-artifact` references. Avoid unless the org route is rejected.
>
> This is a publish-target decision, not a build problem — the built package is
> correct under any of them.

## 4. Develop COMPASS against the real build *before* publishing

The registry decision above does not block COMPASS development. Point COMPASS at
the local build with a `file:` dependency and integrate the real verifier today:

```jsonc
// COMPASS package.json (separate repo — not an ATLAS change)
"dependencies": {
  "@platform/atlas-artifact": "file:../regulatory-rule-engine/crates/ke-wasm"
}
```

Run `scripts/serve-published-registry.sh` for the HTTP registry/verify surface
the browser verifier folds in (`docs/consumer-serve-contract.md`). Swap the
`file:` dep for the published version once step 3's target is decided.

## Honesty boundary

This build signs/verifies with **fixed-seed TEST keys** (`is_test_key:true` in
every provenance record). Publishing makes the verifier *available and real*, not
*production-trusted* — production compiler/expert/registry-root key authority is
the still-open ADR-0009 decision. Keep that disclosure in any consumer UI.
