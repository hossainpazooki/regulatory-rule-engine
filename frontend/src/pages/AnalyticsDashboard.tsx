import { useState } from 'react'
import { useClusters, useCoverage, useConflicts } from '@/hooks'
import { LoadingOverlay, ErrorMessage, MetricCard, StatusBadge } from '@/components/common'

type Tab = 'clusters' | 'coverage' | 'conflicts'

export function AnalyticsDashboard() {
  const [activeTab, setActiveTab] = useState<Tab>('clusters')

  const { data: clusters, isLoading: clustersLoading } = useClusters()
  const { data: coverage, isLoading: coverageLoading } = useCoverage()
  const { data: conflicts, isLoading: conflictsLoading } = useConflicts()

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Analytics Dashboard</h1>
        <p className="text-slate-400">Rule clustering, coverage gaps, and conflict resolution</p>
      </div>

      {/* Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <MetricCard
          title="Clusters"
          value={clusters?.num_clusters || 0}
          subtitle={`Silhouette: ${clusters?.silhouette_score?.toFixed(2) || '—'}`}
        />
        <MetricCard
          title="Coverage"
          value={`${coverage?.overall_coverage_percentage?.toFixed(0) || 0}%`}
          subtitle="Legal sources"
        />
        <MetricCard title="Conflicts" value={conflicts?.conflicts_found || 0} subtitle="Detected" />
        <MetricCard
          title="High Severity"
          value={conflicts?.high_severity_count || 0}
          subtitle="Critical conflicts"
        />
      </div>

      {/* Tabs */}
      <div className="card">
        <div className="border-b border-slate-700 -mx-6 -mt-6 px-6 mb-6">
          <nav className="flex gap-4">
            {(['clusters', 'coverage', 'conflicts'] as Tab[]).map((tab) => (
              <button
                key={tab}
                onClick={() => setActiveTab(tab)}
                className={`py-4 border-b-2 font-medium transition-colors capitalize ${
                  activeTab === tab
                    ? 'border-primary-500 text-primary-400'
                    : 'border-transparent text-slate-400 hover:text-white'
                }`}
              >
                {tab}
              </button>
            ))}
          </nav>
        </div>

        {/* Tab Content */}
        {activeTab === 'clusters' && (
          <div>
            {clustersLoading ? (
              <LoadingOverlay message="Loading clusters..." />
            ) : !clusters ? (
              <ErrorMessage message="Failed to load clusters" />
            ) : (
              <div className="space-y-4">
                <div className="text-sm text-slate-400">
                  Algorithm: {clusters.algorithm} | Embedding: {clusters.embedding_type}
                </div>
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                  {clusters.clusters.map((cluster) => (
                    <div key={cluster.cluster_id} className="p-4 bg-slate-700 rounded-lg">
                      <div className="flex items-center justify-between mb-2">
                        <span className="font-medium text-white">Cluster {cluster.cluster_id}</span>
                        <span className="text-sm text-slate-400">{cluster.size} rules</span>
                      </div>
                      <div className="text-sm text-slate-400 mb-2">
                        Cohesion: {(cluster.cohesion_score * 100).toFixed(0)}%
                      </div>
                      {cluster.keywords.length > 0 && (
                        <div className="flex flex-wrap gap-1">
                          {cluster.keywords.slice(0, 5).map((kw) => (
                            <span
                              key={kw}
                              className="px-2 py-0.5 bg-slate-600 rounded text-xs text-slate-300"
                            >
                              {kw}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        {activeTab === 'coverage' && (
          <div>
            {coverageLoading ? (
              <LoadingOverlay message="Loading coverage..." />
            ) : !coverage ? (
              <ErrorMessage message="Failed to load coverage" />
            ) : (
              <div className="space-y-4">
                <div className="text-sm text-slate-400">
                  {coverage.total_rules} rules covering {coverage.total_legal_sources} legal sources
                </div>
                <div className="space-y-3">
                  {Object.entries(coverage.coverage_by_framework).map(([framework, data]) => (
                    <div key={framework} className="p-4 bg-slate-700 rounded-lg">
                      <div className="flex items-center justify-between mb-2">
                        <span className="font-medium text-white">{framework}</span>
                        <span className="text-primary-400">
                          {data.coverage_percentage.toFixed(0)}%
                        </span>
                      </div>
                      <div className="w-full h-2 bg-slate-600 rounded-full">
                        <div
                          className="h-full bg-primary-500 rounded-full"
                          style={{ width: `${data.coverage_percentage}%` }}
                        />
                      </div>
                      <div className="text-sm text-slate-400 mt-2">
                        {data.covered_articles}/{data.total_articles} articles covered |{' '}
                        {data.rule_count} rules
                      </div>
                    </div>
                  ))}
                </div>
                {coverage.coverage_gaps.length > 0 && (
                  <div>
                    <h3 className="text-lg font-semibold text-white mb-3">Coverage Gaps</h3>
                    <div className="space-y-2">
                      {coverage.coverage_gaps.map((gap, idx) => (
                        <div key={idx} className="p-3 bg-slate-700 rounded-lg">
                          <div className="flex items-center gap-2">
                            <StatusBadge
                              status={
                                gap.importance === 'high'
                                  ? 'error'
                                  : gap.importance === 'medium'
                                    ? 'warning'
                                    : 'info'
                              }
                              label={gap.importance}
                              size="sm"
                            />
                            <span className="text-white">
                              {gap.framework} - {gap.article}
                            </span>
                          </div>
                          <p className="text-sm text-slate-400 mt-1">{gap.recommendation}</p>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {activeTab === 'conflicts' && (
          <div>
            {conflictsLoading ? (
              <LoadingOverlay message="Loading conflicts..." />
            ) : !conflicts ? (
              <ErrorMessage message="Failed to load conflicts" />
            ) : conflicts.conflicts.length === 0 ? (
              <div className="text-center py-12 text-slate-400">No conflicts detected</div>
            ) : (
              <div className="space-y-4">
                <div className="text-sm text-slate-400">
                  Analyzed {conflicts.total_rules_analyzed} rules | Found{' '}
                  {conflicts.conflicts_found} conflicts
                </div>
                <div className="space-y-3">
                  {conflicts.conflicts.map((conflict, idx) => (
                    <div key={idx} className="p-4 bg-slate-700 rounded-lg">
                      <div className="flex items-center justify-between mb-2">
                        <div className="flex items-center gap-2">
                          <StatusBadge
                            status={
                              conflict.severity === 'high'
                                ? 'error'
                                : conflict.severity === 'medium'
                                  ? 'warning'
                                  : 'info'
                            }
                            label={conflict.severity}
                            size="sm"
                          />
                          <span className="text-white font-medium">
                            {conflict.rule1_id} vs {conflict.rule2_id}
                          </span>
                        </div>
                        <span className="text-sm text-slate-400 capitalize">
                          {conflict.conflict_type}
                        </span>
                      </div>
                      <p className="text-slate-300">{conflict.description}</p>
                      {conflict.resolution_hints.length > 0 && (
                        <div className="mt-2 text-sm text-slate-400">
                          <strong>Resolution:</strong> {conflict.resolution_hints[0]}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
