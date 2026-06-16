/**
 * Gate-5 Playwright visual-regression harness — deterministic local-surface
 * fixtures (Workstream E).
 *
 * These MSW handlers mock EVERY network call the rewired (flag-on) pages make so
 * full-page screenshots render with NO live backend. They are written as MSW v2
 * (`msw`) request handlers so the same definitions can drive either:
 *   - the in-browser MSW service worker (the documented service-worker path), or
 *   - Playwright's `page.route()` network interception (the mechanism the spec
 *     actually wires up — see `installNetworkMocks` in `visual.spec.ts`),
 * both of which return the identical JSON.
 *
 * Two origins are mocked, matching the contract's surface assignment:
 *   1. `VITE_API_URL` (baked to `http://msw.local/api` for the visual build) —
 *      the UNTOUCHED fallback path. With `VITE_USE_LOCAL_KE_API=true`, the
 *      rules-list and decide variants are SCAFFOLD-ONLY and fall through to this
 *      path (`ServeUnsupportedError`), so these handlers are what actually render
 *      KEWorkbench's rule list and ProductionDemo's scenarios.
 *   2. `KE_SERVE_URL` (`http://localhost:8787`) — the local `ke-cli serve`
 *      surface (ADR-0018). Genuine rewired consumers (ADR-0020): ProductionDemo
 *      health (`GET /healthz`), and KEWorkbench's `LocalKePreviewPane` —
 *      `POST /compile/preview`, `POST /dry-run`, and `POST /verify` (whose
 *      `provenance` feeds the 5e ReviewSurface). Mocked here so the rewired path is
 *      exercised over serve HTTP; the WASM `.wasm` binary is never loaded in CI
 *      (convention B), since the visual build runs with `VITE_USE_WASM_PREVIEW=false`.
 *
 * Fixtures are FIXED (no randomness, no clocks) so Linux baselines are stable.
 */
import { http, HttpResponse } from 'msw'

/** Origin the visual build bakes into `VITE_API_URL`. Must match the env passed
 *  to `npm run build` in the CI job and the local preview env. */
export const VISUAL_API_URL = 'http://msw.local/api'
/** Local serve surface origin (matches `KE_SERVE_URL` default). */
export const VISUAL_SERVE_URL = 'http://localhost:8787'

/** Deterministic rule corpus rendered by KEWorkbench (left list) and used by
 *  ProductionDemo to generate synthetic scenarios. */
const RULES = [
  { rule_id: 'mica-art-16-reserve', description: 'MiCA Art. 16 reserve-of-assets requirement', framework: 'MiCA' },
  { rule_id: 'mica-art-4-public-offer', description: 'MiCA Art. 4 public offer authorisation', framework: 'MiCA' },
  { rule_id: 'fca-ps22-10-registration', description: 'FCA PS22/10 cryptoasset registration', framework: 'FCA' },
  { rule_id: 'genius-act-stablecoin', description: 'GENIUS Act payment stablecoin issuer', framework: 'GENIUS' },
  { rule_id: 'finma-dlt-custody', description: 'FINMA DLT custody obligations', framework: 'FINMA' },
]

const RULES_LIST = { rules: RULES, total: RULES.length }

const DATABASE_STATS = {
  rules_count: RULES.length,
  compiled_rules_count: RULES.length,
  verification_stats: { passed: RULES.length, failed: 0 },
  reviews_count: 3,
  premise_keys_count: 12,
}

const CACHE_STATS = { size: 42, hits: 1280, misses: 96, hit_rate: 0.93 }

const SYSTEM_CONFIG = {
  features: {
    rate_limiting: true,
    rate_limit: '100/min',
    audit_logging: true,
    tracing: true,
    auth_required: false,
  },
  observability: {
    log_format: 'json',
    log_level: 'info',
    service_name: 'ke-workbench-frontend',
  },
}

const DECIDE_RESPONSE = {
  results: [
    {
      rule_id: 'mica-art-16-reserve',
      outcome: 'requires_review',
      trace: [
        { node: 'n0', condition: 'instrument_type == art', result: true },
        { node: 'n1', condition: 'authorized == true', result: false },
      ],
    },
  ],
}

/** A decision tree for `GET /ke/charts/decision-tree/:id`. */
const DECISION_TREE = {
  data: {
    id: 'root',
    title: 'instrument_type == art',
    type: 'branch',
    condition: 'instrument_type == art',
    children: [
      { id: 'y', branch: 'true', title: 'authorized == true', type: 'branch', condition: 'authorized == true' },
      { id: 'n', branch: 'false', title: 'not in scope', type: 'leaf', result: 'out_of_scope' },
    ],
  },
}

/** serve `GET /healthz` body (dto.rs `HealthResponse`). */
const SERVE_HEALTH = { ok: true, surface: 'ke-cli serve (preview, non-authoritative)' }

/** serve `POST /compile/preview` body — `CompilePreviewResult` (`@/wasm`). */
const COMPILE_PREVIEW = {
  rules: [],
  report: { has_blocking: false, findings: [], conflicts: [] },
}

/** serve `POST /dry-run` body — `DryRunResult` (`@/wasm`). */
const DRY_RUN_RESULT = { evaluations: [] }

/** Canonical `ArtifactProvenance` (serveClient.ts) — fed to the 5e ReviewSurface.
 *  Fixed (test key, fixed clock) so the Linux baseline is stable. */
const PROVENANCE = {
  regime_id: 'mica_2023',
  artifact_hash: [0x12, 0x34, 0x56, 0x78],
  ir_schema_version: 'ir-1',
  codec_version: 'codec-1',
  canonicalization_version: 'canon-1',
  signer_key_id: 'test-fixed-seed-1',
  is_test_key: true,
  attestations: [
    {
      attestation_type: 'SourceFidelity',
      signer_key_id: 'expert-1',
      is_test_key: true,
      tsa_class: 'rfc3161',
      claimed_time_unix: 1_700_000_100,
    },
  ],
  registry_state: 'Published',
  registry_event_head_hash: [0x00],
  exported_at_unix: 1_700_000_000,
}

/** serve `POST /verify` body — `VerifyResponse` (serveClient.ts). */
const VERIFY_RESPONSE = {
  verdict: 'verified',
  provenance: PROVENANCE,
  registry_state: 'Published',
}

/**
 * Plain route table consumed by Playwright `page.route()` in the spec. Each entry
 * is a glob (Playwright URL pattern) + the JSON body to fulfill with. This is the
 * authoritative fixture data; the MSW `handlers` below wrap the SAME bodies.
 */
export const routeTable: Array<{ pattern: string; json: unknown }> = [
  // --- VITE_API_URL fallback surface (rendered for both rewired pages) ---
  { pattern: '**/api/rules', json: RULES_LIST },
  { pattern: '**/api/rules/*/versions', json: { versions: [] } },
  { pattern: '**/api/rules/*/events', json: { events: [] } },
  { pattern: '**/api/rules/*', json: { rule_id: RULES[0].rule_id, description: RULES[0].description, decision_tree: DECISION_TREE.data } },
  { pattern: '**/api/ke/charts/decision-tree/**', json: DECISION_TREE },
  { pattern: '**/api/decide', json: DECIDE_RESPONSE },
  { pattern: '**/api/health', json: { status: 'healthy' } },
  { pattern: '**/api/v2/status', json: DATABASE_STATS },
  { pattern: '**/api/v2/cache/stats', json: CACHE_STATS },
  { pattern: '**/api/v2/config', json: SYSTEM_CONFIG },
  // --- KE_SERVE_URL local serve surface (ADR-0018; consumers per ADR-0020) ---
  { pattern: 'http://localhost:8787/healthz', json: SERVE_HEALTH },
  { pattern: 'http://localhost:8787/dry-run', json: DRY_RUN_RESULT },
  { pattern: 'http://localhost:8787/compile/preview', json: COMPILE_PREVIEW },
  { pattern: 'http://localhost:8787/verify', json: VERIFY_RESPONSE },
]

/**
 * MSW v2 handlers (service-worker path). These mirror `routeTable` exactly so the
 * documented `mockServiceWorker.js` flow returns identical bodies. Kept in sync
 * with `routeTable`; the spec uses `page.route()` for reliability in headless CI.
 */
export const handlers = [
  http.get(`${VISUAL_API_URL}/rules`, () => HttpResponse.json(RULES_LIST)),
  http.get(`${VISUAL_API_URL}/rules/:id/versions`, () => HttpResponse.json({ versions: [] })),
  http.get(`${VISUAL_API_URL}/rules/:id/events`, () => HttpResponse.json({ events: [] })),
  http.get(`${VISUAL_API_URL}/rules/:id`, ({ params }) =>
    HttpResponse.json({ rule_id: String(params.id), description: RULES[0].description, decision_tree: DECISION_TREE.data }),
  ),
  http.get(`${VISUAL_API_URL}/ke/charts/decision-tree/:id`, () => HttpResponse.json(DECISION_TREE)),
  http.post(`${VISUAL_API_URL}/decide`, () => HttpResponse.json(DECIDE_RESPONSE)),
  http.get(`${VISUAL_API_URL}/health`, () => HttpResponse.json({ status: 'healthy' })),
  http.get(`${VISUAL_API_URL}/v2/status`, () => HttpResponse.json(DATABASE_STATS)),
  http.get(`${VISUAL_API_URL}/v2/cache/stats`, () => HttpResponse.json(CACHE_STATS)),
  http.get(`${VISUAL_API_URL}/v2/config`, () => HttpResponse.json(SYSTEM_CONFIG)),
  http.get(`${VISUAL_SERVE_URL}/healthz`, () => HttpResponse.json(SERVE_HEALTH)),
  http.post(`${VISUAL_SERVE_URL}/dry-run`, () => HttpResponse.json(DRY_RUN_RESULT)),
  http.post(`${VISUAL_SERVE_URL}/compile/preview`, () => HttpResponse.json(COMPILE_PREVIEW)),
  http.post(`${VISUAL_SERVE_URL}/verify`, () => HttpResponse.json(VERIFY_RESPONSE)),
]
