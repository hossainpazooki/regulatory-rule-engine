import { useState } from 'react'
import { useRules, useSimilarRules } from '@/hooks'
import { useAnalyticsStore } from '@/store'
import { LoadingOverlay, ErrorMessage, MetricCard } from '@/components/common'
import type { EmbeddingType } from '@/types'

export function SimilaritySearch() {
  const { data: rulesData, isLoading: rulesLoading } = useRules()
  const [selectedRuleId, setSelectedRuleId] = useState<string>('')
  const { searchWeights, setSearchWeight, resetSearchWeights } = useAnalyticsStore()

  const {
    data: similarRules,
    isLoading: searchLoading,
    error: searchError,
  } = useSimilarRules({
    rule_id: selectedRuleId,
    embedding_type: 'all',
    top_k: 10,
    min_score: 0.3,
    include_explanation: true,
  })

  const embeddingTypes: EmbeddingType[] = ['semantic', 'structural', 'entity', 'legal']

  if (rulesLoading) return <LoadingOverlay message="Loading rules..." />

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Similarity Search</h1>
        <p className="text-slate-400">Find related rules across jurisdictions with weighted search</p>
      </div>

      {/* Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <MetricCard title="Query Rule" value={selectedRuleId || '—'} subtitle="Selected" />
        <MetricCard
          title="Results"
          value={similarRules?.similar_rules.length || 0}
          subtitle="Similar rules found"
        />
        <MetricCard
          title="Candidates"
          value={similarRules?.total_candidates || 0}
          subtitle="Total searched"
        />
        <MetricCard title="Min Score" value="0.3" subtitle="Threshold" />
      </div>

      <div className="grid grid-cols-12 gap-6">
        {/* Search Controls */}
        <div className="col-span-4 space-y-4">
          {/* Rule Selection */}
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Select Rule</h2>
            <select
              value={selectedRuleId}
              onChange={(e) => setSelectedRuleId(e.target.value)}
              className="input w-full"
            >
              <option value="">Select a rule...</option>
              {rulesData?.rules.map((rule) => (
                <option key={rule.rule_id} value={rule.rule_id}>
                  {rule.rule_id}
                </option>
              ))}
            </select>
          </div>

          {/* Weight Sliders */}
          <div className="card">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-lg font-semibold text-white">Weights</h2>
              <button onClick={resetSearchWeights} className="text-sm text-primary-400 hover:underline">
                Reset
              </button>
            </div>
            <div className="space-y-4">
              {embeddingTypes.map((type) => (
                <div key={type}>
                  <div className="flex items-center justify-between mb-1">
                    <label className="text-sm text-slate-400 capitalize">{type}</label>
                    <span className="text-sm text-slate-300">
                      {(searchWeights[type] * 100).toFixed(0)}%
                    </span>
                  </div>
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.05"
                    value={searchWeights[type]}
                    onChange={(e) => setSearchWeight(type, parseFloat(e.target.value))}
                    className="w-full accent-primary-500"
                  />
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* Results */}
        <div className="col-span-8">
          <div className="card min-h-[500px]">
            <h2 className="text-lg font-semibold text-white mb-4">Similar Rules</h2>

            {!selectedRuleId ? (
              <div className="text-center py-12 text-slate-400">
                Select a rule to find similar ones
              </div>
            ) : searchLoading ? (
              <LoadingOverlay message="Searching..." />
            ) : searchError ? (
              <ErrorMessage message="Search failed" />
            ) : similarRules?.similar_rules.length === 0 ? (
              <div className="text-center py-12 text-slate-400">No similar rules found</div>
            ) : (
              <div className="space-y-3">
                {similarRules?.similar_rules.map((rule) => (
                  <div
                    key={rule.rule_id}
                    className="p-4 bg-slate-700 rounded-lg hover:bg-slate-600 transition-colors"
                  >
                    <div className="flex items-start justify-between">
                      <div>
                        <p className="font-medium text-white">{rule.rule_id}</p>
                        {rule.jurisdiction && (
                          <p className="text-sm text-slate-400">{rule.jurisdiction}</p>
                        )}
                      </div>
                      <div className="text-right">
                        <p className="text-lg font-semibold text-primary-400">
                          {(rule.overall_score * 100).toFixed(0)}%
                        </p>
                        <p className="text-xs text-slate-500">similarity</p>
                      </div>
                    </div>
                    {rule.explanation && (
                      <p className="text-sm text-slate-300 mt-2">
                        {rule.explanation.primary_reason}
                      </p>
                    )}
                    <div className="flex gap-2 mt-2">
                      {Object.entries(rule.scores_by_type).map(([type, score]) => (
                        <span
                          key={type}
                          className="px-2 py-0.5 bg-slate-800 rounded text-xs text-slate-400"
                        >
                          {type}: {((score as number) * 100).toFixed(0)}%
                        </span>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
