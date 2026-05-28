import { useState } from 'react'
import { useUMAPProjection, useClusters } from '@/hooks'
import { useAnalyticsStore } from '@/store'
import { LoadingOverlay, ErrorMessage, MetricCard } from '@/components/common'
import { UMAPScatter } from '@/components/visualizations'
import type { EmbeddingType, UMAPPoint } from '@/types'

const embeddingTypes: { value: EmbeddingType; label: string; description: string }[] = [
  { value: 'semantic', label: 'Semantic', description: 'Meaning and intent of rules' },
  { value: 'structural', label: 'Structural', description: 'Decision tree structure' },
  { value: 'entity', label: 'Entity', description: 'Field names and operators' },
  { value: 'legal', label: 'Legal', description: 'Citations and sources' },
]

type ColorMode = 'jurisdiction' | 'cluster'

export function EmbeddingExplorer() {
  const [nComponents, setNComponents] = useState<2 | 3>(2)
  const [colorBy, setColorBy] = useState<ColorMode>('jurisdiction')
  const { selectedEmbeddingType, setSelectedEmbeddingType, selectedPoint, setSelectedPoint, selectedCluster, setSelectedCluster } =
    useAnalyticsStore()

  const {
    data: projection,
    isLoading,
    error,
  } = useUMAPProjection({
    embedding_type: selectedEmbeddingType,
    n_components: nComponents,
  })

  const { data: clusters } = useClusters()

  // Get unique clusters from points
  const clusterIds = projection?.points
    ? [...new Set(projection.points.map((p) => p.cluster_id).filter((c) => c !== undefined))]
    : []

  const handlePointClick = (point: UMAPPoint) => {
    setSelectedPoint(point.rule_id === selectedPoint?.rule_id ? null : point)
  }

  const handleBrushEnd = (points: UMAPPoint[]) => {
    if (points.length === 1) {
      setSelectedPoint(points[0])
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Embedding Explorer</h1>
        <p className="text-slate-400">UMAP visualization of rule embeddings by type</p>
      </div>

      {/* Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <MetricCard
          title="Embedding Type"
          value={selectedEmbeddingType}
          subtitle="Current selection"
        />
        <MetricCard title="Dimensions" value={nComponents === 2 ? '2D' : '3D'} subtitle="Projection" />
        <MetricCard title="Points" value={projection?.total_rules || 0} subtitle="Rules plotted" />
        <MetricCard
          title="Selected"
          value={selectedPoint?.rule_id || '—'}
          subtitle="Current point"
        />
      </div>

      <div className="grid grid-cols-12 gap-6">
        {/* Controls */}
        <div className="col-span-3 space-y-4">
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Embedding Type</h2>
            <div className="space-y-2">
              {embeddingTypes.map((type) => (
                <button
                  key={type.value}
                  onClick={() => setSelectedEmbeddingType(type.value)}
                  className={`w-full text-left p-3 rounded-lg transition-colors ${
                    selectedEmbeddingType === type.value
                      ? 'bg-primary-600 text-white'
                      : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                  }`}
                >
                  <p className="font-medium">{type.label}</p>
                  <p className="text-sm opacity-75">{type.description}</p>
                </button>
              ))}
            </div>

            <div className="pt-4 border-t border-slate-700 mt-4">
              <h3 className="text-sm font-medium text-slate-400 mb-2">Dimensions</h3>
              <div className="flex gap-2">
                <button
                  onClick={() => setNComponents(2)}
                  className={`flex-1 py-2 rounded-lg font-medium transition-colors ${
                    nComponents === 2
                      ? 'bg-primary-600 text-white'
                      : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                  }`}
                >
                  2D
                </button>
                <button
                  onClick={() => setNComponents(3)}
                  className={`flex-1 py-2 rounded-lg font-medium transition-colors ${
                    nComponents === 3
                      ? 'bg-primary-600 text-white'
                      : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                  }`}
                >
                  3D
                </button>
              </div>
            </div>
          </div>

          {/* Color Settings */}
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Display Settings</h2>
            <div className="space-y-4">
              <div>
                <label className="block text-sm text-slate-400 mb-2">Color By</label>
                <select
                  value={colorBy}
                  onChange={(e) => setColorBy(e.target.value as ColorMode)}
                  className="input w-full"
                >
                  <option value="jurisdiction">Jurisdiction</option>
                  <option value="cluster">Cluster</option>
                </select>
              </div>

              {colorBy === 'cluster' && clusterIds.length > 0 && (
                <div>
                  <label className="block text-sm text-slate-400 mb-2">Highlight Cluster</label>
                  <select
                    value={selectedCluster ?? ''}
                    onChange={(e) => setSelectedCluster(e.target.value ? Number(e.target.value) : null)}
                    className="input w-full"
                  >
                    <option value="">All Clusters</option>
                    {clusterIds.map((id) => (
                      <option key={id} value={id}>
                        Cluster {id}
                      </option>
                    ))}
                  </select>
                </div>
              )}
            </div>
          </div>

          {/* Selected Point Info */}
          {selectedPoint && (
            <div className="card">
              <h2 className="text-lg font-semibold text-white mb-4">Selected Point</h2>
              <div className="space-y-2 text-sm">
                <p className="text-slate-300">
                  <span className="text-slate-400">Rule ID:</span> {selectedPoint.rule_id}
                </p>
                {selectedPoint.rule_name && (
                  <p className="text-slate-300">
                    <span className="text-slate-400">Name:</span> {selectedPoint.rule_name}
                  </p>
                )}
                <p className="text-slate-300">
                  <span className="text-slate-400">X:</span> {selectedPoint.x.toFixed(4)}
                </p>
                <p className="text-slate-300">
                  <span className="text-slate-400">Y:</span> {selectedPoint.y.toFixed(4)}
                </p>
                {selectedPoint.z !== undefined && (
                  <p className="text-slate-300">
                    <span className="text-slate-400">Z:</span> {selectedPoint.z.toFixed(4)}
                  </p>
                )}
                {selectedPoint.jurisdiction && (
                  <p className="text-slate-300">
                    <span className="text-slate-400">Jurisdiction:</span> {selectedPoint.jurisdiction}
                  </p>
                )}
                {selectedPoint.cluster_id !== undefined && (
                  <p className="text-slate-300">
                    <span className="text-slate-400">Cluster:</span> {selectedPoint.cluster_id}
                  </p>
                )}
                <button
                  onClick={() => setSelectedPoint(null)}
                  className="btn-secondary w-full mt-3"
                >
                  Clear Selection
                </button>
              </div>
            </div>
          )}

          {/* Cluster Stats */}
          {clusters && (
            <div className="card">
              <h2 className="text-lg font-semibold text-white mb-4">Cluster Stats</h2>
              <div className="space-y-2 text-sm">
                <p className="text-slate-300">
                  <span className="text-slate-400">Clusters:</span> {clusters.num_clusters}
                </p>
                <p className="text-slate-300">
                  <span className="text-slate-400">Algorithm:</span> {clusters.algorithm}
                </p>
                <p className="text-slate-300">
                  <span className="text-slate-400">Silhouette:</span> {clusters.silhouette_score?.toFixed(3) || '—'}
                </p>
              </div>
            </div>
          )}
        </div>

        {/* Scatter Plot */}
        <div className="col-span-9">
          <div className="card min-h-[600px] flex flex-col">
            <h2 className="text-lg font-semibold text-white mb-4">UMAP Projection</h2>

            {isLoading ? (
              <div className="flex-1 flex items-center justify-center">
                <LoadingOverlay message="Computing UMAP projection..." />
              </div>
            ) : error ? (
              <ErrorMessage message="Failed to load projection" />
            ) : !projection || projection.points.length === 0 ? (
              <div className="flex-1 flex items-center justify-center text-slate-400">
                No projection data available
              </div>
            ) : (
              <div className="flex-1">
                <UMAPScatter
                  data={projection.points}
                  width={800}
                  height={550}
                  colorBy={colorBy}
                  selectedPoint={selectedPoint?.rule_id}
                  highlightedCluster={selectedCluster}
                  onPointClick={handlePointClick}
                  onBrushEnd={handleBrushEnd}
                />
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
