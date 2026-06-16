// SCAFFOLD-ONLY (not-yet-rewired): the `ke-cli serve` surface (ADR-0018) exposes
// only /healthz, /resolve, /verify, /compile/preview, /dry-run, /events. There is
// NO rule-graph / network-graph endpoint, and no WASM-preview equivalent: the
// Node2Vec rule-relationship graph (single-rule and full-network views) is an
// ML-derived embedding artifact computed off the ATLAS artifact path. Per the
// Gate-5 contract (§ B.2 / mapping table: GraphVisualizer = SCAFFOLD-ONLY), each
// variant is wired flag-and-fallback: it throws `ServeUnsupportedError` so the
// hook selector (`selectQueryFn`) transparently falls back to the untouched
// `VITE_API_URL` path. With `VITE_USE_LOCAL_KE_API` ON, GraphVisualizer behaves
// EXACTLY as it does on `main`. These MUST NOT silently return an empty graph —
// that would hide the not-yet-rewired boundary and risk presenting a
// non-authoritative/empty preview as real graph data.
//
// Note: these GraphVisualizer variants live in their OWN module (not the shared
// `analytics.serve.ts`) so the GraphVisualizer page rewire owns a disjoint file
// and does not collide with the other analytics-backed pages' variants.

import type { GraphData } from '@/types'
import { ServeUnsupportedError } from './serveClient'

const PAGE = 'GraphVisualizer'
const REASON =
  'ke-cli serve (ADR-0018) exposes no rule-graph / network-graph endpoint ' +
  '(Node2Vec embeddings are ML-derived, off the ATLAS artifact path) and there ' +
  'is no WASM-preview equivalent; graph visualization is not yet rewired'

/**
 * LOCAL variant of `analyticsApi.getGraph` — IDENTICAL TypeScript signature
 * (`(ruleId?: string) => Promise<GraphData>`).
 *
 * SCAFFOLD-ONLY: always throws `ServeUnsupportedError`. The hook (`useRuleGraph`)
 * catches it via `selectQueryFn` and re-runs the canonical `analyticsApi.getGraph`
 * fallback, so enabling `VITE_USE_LOCAL_KE_API` does not change the single-rule
 * graph view — it merely marks the call as wired-for-rewire with an honest
 * not-yet-implemented local surface. The `_ruleId` argument is accepted to keep
 * the signature identical to the canonical method.
 */
export async function getGraphLocal(_ruleId?: string): Promise<GraphData> {
  throw new ServeUnsupportedError(`${PAGE} (analyticsApi.getGraph)`, REASON)
}

/**
 * LOCAL variant of `analyticsApi.getNetworkGraph` — IDENTICAL TypeScript
 * signature (`(minSimilarity?: number) => Promise<GraphData>`).
 *
 * SCAFFOLD-ONLY: always throws `ServeUnsupportedError`; the hook
 * (`useNetworkGraph`) falls back to the canonical `VITE_API_URL` path. The
 * `_minSimilarity` argument is accepted to keep the signature identical to the
 * canonical method.
 */
export async function getNetworkGraphLocal(_minSimilarity = 0.7): Promise<GraphData> {
  throw new ServeUnsupportedError(`${PAGE} (analyticsApi.getNetworkGraph)`, REASON)
}
