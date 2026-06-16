/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Existing fallback backend base URL. Untouched by Gate 5; used when all
   *  Gate-5 flags are OFF (the default). */
  readonly VITE_API_URL?: string

  // --- Gate-5 feature flags (spec § 7.4). ALL DEFAULT-OFF. ---
  /** Route a page's REST call to `ke-cli serve` instead of VITE_API_URL. */
  readonly VITE_USE_LOCAL_KE_API?: string
  /** Use the frontend/src/wasm adapter for compile-preview / dry-run. */
  readonly VITE_USE_WASM_PREVIEW?: string
  /** Show the 5e AI-provenance review UI. */
  readonly VITE_USE_REVIEW_UI?: string
  /** Base URL for the local ke-cli serve surface (default http://localhost:8787). */
  readonly VITE_KE_SERVE_URL?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
