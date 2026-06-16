// SCAFFOLD-ONLY (not-yet-rewired): the `ke-cli serve` surface (ADR-0018) exposes
// only /healthz, /resolve, /verify, /compile/preview, /dry-run, /events. NONE of
// the analytics computations behind the AnalyticsDashboard, EmbeddingExplorer, and
// SimilaritySearch pages — clustering, legal-source coverage, cross-rule conflict
// detection, UMAP projection, and similarity search — has a serve endpoint or a
// WASM-preview equivalent (they are ML/analytics-derived, off the ATLAS artifact
// path). Per the Gate-5 contract (§ B.2 / mapping table), each `*Local` variant is
// flag-and-fallback wired but its body throws `ServeUnsupportedError` so the
// consuming hook transparently falls back to the untouched `VITE_API_URL` path;
// with the flag on, every analytics page behaves EXACTLY as on `main`. These MUST
// NOT silently return empty / fabricated analytics — that would hide the
// not-yet-rewired boundary and risk presenting non-authoritative preview data as
// authoritative.
//
// NOTE (multi-page shared file): the `analyticsApi` object backs several pages, so
// this one variant module collects every analytics scaffold variant EXCEPT the
// rule/network graphs, which live in `graph.serve.ts`. Add new variants here
// additively — do not overwrite the file.
import type {
  ClusterRequest,
  ClusterAnalysis,
  ConflictSearchRequest,
  ConflictReport,
  CoverageReport,
  UMAPProjectionRequest,
  UMAPProjectionResponse,
  SimilarRulesRequest,
  SimilarRulesResponse,
} from '@/types'
import { ServeUnsupportedError } from './serveClient'

// --- AnalyticsDashboard (analyticsApi.cluster / getCoverage / findConflicts) ---

/**
 * LOCAL variant of `analyticsApi.cluster` — IDENTICAL return type
 * (`Promise<ClusterAnalysis>`). SCAFFOLD-ONLY: always throws
 * `ServeUnsupportedError`; the hook (`useClusters`) falls back to the canonical
 * `analyticsApi.cluster` path, so `VITE_USE_LOCAL_KE_API` does not change the
 * AnalyticsDashboard Clusters tab.
 */
export async function clusterLocal(_request: ClusterRequest = {}): Promise<ClusterAnalysis> {
  throw new ServeUnsupportedError(
    'AnalyticsDashboard (analyticsApi.cluster)',
    'ke-cli serve (ADR-0018) exposes no clustering endpoint (clustering is ML-derived, ' +
      'off the ATLAS artifact path) and there is no WASM-preview equivalent; not yet rewired',
  )
}

/**
 * LOCAL variant of `analyticsApi.getCoverage` — IDENTICAL return type
 * (`Promise<CoverageReport>`). SCAFFOLD-ONLY: always throws; the hook
 * (`useCoverage`) falls back to the canonical `VITE_API_URL` path.
 */
export async function getCoverageLocal(): Promise<CoverageReport> {
  throw new ServeUnsupportedError(
    'AnalyticsDashboard (analyticsApi.getCoverage)',
    'ke-cli serve (ADR-0018) exposes no coverage endpoint (legal-source coverage is ' +
      'analytics-derived, off the ATLAS artifact path) and there is no WASM-preview ' +
      'equivalent; not yet rewired',
  )
}

/**
 * LOCAL variant of `analyticsApi.findConflicts` — IDENTICAL return type
 * (`Promise<ConflictReport>`). SCAFFOLD-ONLY: always throws; the hook
 * (`useConflicts`) falls back to the canonical `VITE_API_URL` path.
 */
export async function findConflictsLocal(
  _request: ConflictSearchRequest = {},
): Promise<ConflictReport> {
  throw new ServeUnsupportedError(
    'AnalyticsDashboard (analyticsApi.findConflicts)',
    'ke-cli serve (ADR-0018) exposes no conflict-detection endpoint (conflict detection ' +
      'is ML-derived, off the ATLAS artifact path) and there is no WASM-preview ' +
      'equivalent; not yet rewired',
  )
}

// --- EmbeddingExplorer (analyticsApi.getUMAPProjection) ---

/**
 * LOCAL variant of `analyticsApi.getUMAPProjection` — IDENTICAL return type
 * (`Promise<UMAPProjectionResponse>`). SCAFFOLD-ONLY: always throws; the hook
 * (`useUMAPProjection`) falls back to the canonical `VITE_API_URL` path.
 */
export async function getUMAPProjectionLocal(
  _request: UMAPProjectionRequest = {},
): Promise<UMAPProjectionResponse> {
  throw new ServeUnsupportedError(
    'EmbeddingExplorer (analyticsApi.getUMAPProjection)',
    'ke-cli serve (ADR-0018) exposes no UMAP-projection endpoint (embedding projection ' +
      'is ML-derived, off the ATLAS artifact path) and there is no WASM-preview ' +
      'equivalent; not yet rewired',
  )
}

// --- SimilaritySearch (analyticsApi.findSimilar) ---

/**
 * LOCAL variant of `analyticsApi.findSimilar` — IDENTICAL return type
 * (`Promise<SimilarRulesResponse>`). SCAFFOLD-ONLY: always throws; the hook
 * (`useSimilarRules`) falls back to the canonical `VITE_API_URL` path.
 */
export async function findSimilarLocal(
  _request: SimilarRulesRequest,
): Promise<SimilarRulesResponse> {
  throw new ServeUnsupportedError(
    'SimilaritySearch',
    'serve exposes no similarity-search endpoint and there is no WASM equivalent',
  )
}
