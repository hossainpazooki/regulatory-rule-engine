/**
 * Gate-5 (5d) flag-gated, NON-AUTHORITATIVE local-surface preview pane for
 * KEWorkbench. Returns `null` unless `USE_WASM_PREVIEW` or `USE_LOCAL_KE_API` is
 * on, so with flags off it renders nothing and KEWorkbench is byte-unchanged vs
 * `main`.
 *
 * Genuine local affordances over the ATLAS artifact / rule-engine surface:
 *   - compile-preview + dry-run of INLINE YAML via the WASM adapter (when
 *     `USE_WASM_PREVIEW` is on) or serve `POST /compile/preview` // `/dry-run`.
 *   - verify an artifact by hash via serve `POST /verify`, feeding the 5e
 *     `ReviewSurface` with CANONICAL provenance (`USE_REVIEW_UI` gates the surface
 *     itself).
 *
 * Preview/verify only — never signs, attests, or publishes (spec § 6/§ 16;
 * CLAUDE.md authority boundary). Any divergence from the canonical compile is
 * surfaced to the user, never silently treated as authoritative.
 */
import { useState } from 'react'
import { USE_WASM_PREVIEW, USE_LOCAL_KE_API } from '@/config/flags'
import { useCompilePreview, useDryRun, useVerify } from '@/hooks'
import { ReviewSurface } from '@/components/review/ReviewSurface'
import { StatusBadge } from '@/components/common'

export function LocalKePreviewPane() {
  // Hidden unless a Gate-5 surface flag is on → flag-off KEWorkbench is unchanged.
  if (!USE_WASM_PREVIEW && !USE_LOCAL_KE_API) return null
  return <LocalKePreviewPaneInner />
}

function LocalKePreviewPaneInner() {
  const [source, setSource] = useState('')
  const [factsText, setFactsText] = useState('{}')
  const [factsError, setFactsError] = useState<string | null>(null)
  const [hash, setHash] = useState('')

  const compile = useCompilePreview()
  const dryRun = useDryRun()
  const verify = useVerify()

  // Which non-authoritative surface a preview hits (for the badge).
  const surface = USE_WASM_PREVIEW ? 'WASM preview' : 'serve'

  const handleDryRun = () => {
    setFactsError(null)
    let facts: unknown
    try {
      facts = factsText.trim() ? JSON.parse(factsText) : {}
    } catch {
      setFactsError('Facts must be valid JSON')
      return
    }
    dryRun.mutate({ source, facts })
  }

  return (
    <div className="card" data-testid="local-ke-preview">
      <div className="flex items-center justify-between mb-2">
        <h2 className="text-lg font-semibold text-white">Local KE preview</h2>
        <StatusBadge status="info" label={`non-authoritative · ${surface}`} size="sm" />
      </div>
      <p className="text-xs text-slate-400 mb-3">
        Compile / dry-run inline YAML and verify artifacts against the local Rust
        surfaces. Preview only — never signs, attests, or publishes (spec § 6/§ 16).
      </p>

      <label className="block text-sm text-slate-400 mb-1">Rule source (YAML)</label>
      <textarea
        value={source}
        onChange={(e) => setSource(e.target.value)}
        rows={6}
        className="input w-full font-mono text-xs"
        placeholder="paste rule YAML…"
      />

      <div className="flex gap-2 mt-3">
        <button
          onClick={() => compile.mutate(source)}
          disabled={!source.trim() || compile.isPending}
          className="btn-primary"
        >
          {compile.isPending ? 'Compiling…' : 'Compile preview'}
        </button>
        <button
          onClick={handleDryRun}
          disabled={!source.trim() || dryRun.isPending}
          className="btn-secondary"
        >
          {dryRun.isPending ? 'Running…' : 'Dry-run'}
        </button>
      </div>

      {compile.error && (
        <p className="text-sm text-red-400 mt-2">{(compile.error as Error).message}</p>
      )}
      {compile.data && (
        <div className="mt-3" data-testid="compile-result">
          <StatusBadge
            status={compile.data.report.has_blocking ? 'error' : 'success'}
            label={compile.data.report.has_blocking ? 'blocking findings' : 'no blocking findings'}
            size="sm"
          />
          {compile.data.report.findings.length > 0 && (
            <ul className="mt-2 space-y-1 text-xs text-slate-300">
              {compile.data.report.findings.map((f, i) => (
                <li key={i}>
                  [{f.tier}] {f.code} — {f.message}
                  {f.rule_id ? ` (${f.rule_id})` : ''}
                </li>
              ))}
            </ul>
          )}
          {compile.data.report.conflicts.length > 0 && (
            <ul className="mt-2 space-y-1 text-xs text-amber-300">
              {compile.data.report.conflicts.map((c, i) => (
                <li key={i}>
                  {c.class}/{c.severity}: {c.message}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      <label className="block text-sm text-slate-400 mb-1 mt-4">Facts (JSON)</label>
      <textarea
        value={factsText}
        onChange={(e) => setFactsText(e.target.value)}
        rows={3}
        className="input w-full font-mono text-xs"
      />
      {factsError && <p className="text-sm text-red-400 mt-1">{factsError}</p>}
      {dryRun.error && (
        <p className="text-sm text-red-400 mt-2">{(dryRun.error as Error).message}</p>
      )}
      {dryRun.data && (
        <pre
          className="mt-2 bg-slate-900 p-3 rounded text-xs overflow-auto max-h-60"
          data-testid="dryrun-result"
        >
          {JSON.stringify(dryRun.data.evaluations, null, 2)}
        </pre>
      )}

      <label className="block text-sm text-slate-400 mb-1 mt-4">Verify artifact by hash</label>
      <div className="flex gap-2">
        <input
          value={hash}
          onChange={(e) => setHash(e.target.value)}
          className="input flex-1 font-mono text-xs"
          placeholder="64-hex artifact hash"
        />
        <button
          onClick={() => verify.mutate({ hash })}
          disabled={!hash.trim() || verify.isPending}
          className="btn-secondary"
        >
          {verify.isPending ? 'Verifying…' : 'Verify'}
        </button>
      </div>
      {verify.error && (
        <p className="text-sm text-red-400 mt-2">{(verify.error as Error).message}</p>
      )}
      {verify.data && (
        <div className="mt-3" data-testid="verify-result">
          <StatusBadge
            status={verify.data.verdict === 'verified' ? 'success' : 'error'}
            label={verify.data.verdict + (verify.data.rejection ? `: ${verify.data.rejection}` : '')}
            size="sm"
          />
          {/* 5e review surface — canonical provenance only (USE_REVIEW_UI gates it). */}
          <div className="mt-3">
            <ReviewSurface provenance={verify.data.provenance} />
          </div>
        </div>
      )}
    </div>
  )
}
