import { useState, useMemo } from 'react'
import { useRules, useRuleGraph, useNetworkGraph } from '@/hooks'
import { LoadingOverlay, ErrorMessage, MetricCard } from '@/components/common'
import { ForceGraph } from '@/components/visualizations'
import type { GraphNode, GraphData } from '@/types'

type ViewMode = 'single' | 'network' | 'comparison'
type ColorMode = 'jurisdiction' | 'cluster' | 'type'

export function GraphVisualizer() {
  const [viewMode, setViewMode] = useState<ViewMode>('network')
  const [colorBy, setColorBy] = useState<ColorMode>('jurisdiction')
  const [selectedRuleId, setSelectedRuleId] = useState<string>('')
  const [minSimilarity, setMinSimilarity] = useState(0.7)
  const [showLabels, setShowLabels] = useState(true)
  const [selectedNode, setSelectedNode] = useState<GraphNode | null>(null)

  const { data: rulesData, isLoading: rulesLoading } = useRules()
  const {
    data: ruleGraph,
    isLoading: graphLoading,
    error: graphError,
  } = useRuleGraph(viewMode === 'single' ? selectedRuleId : undefined)
  const {
    data: networkGraph,
    isLoading: networkLoading,
    error: networkError,
  } = useNetworkGraph(minSimilarity)

  const currentGraph = viewMode === 'network' ? networkGraph : ruleGraph
  const isLoading = viewMode === 'network' ? networkLoading : graphLoading
  const error = viewMode === 'network' ? networkError : graphError

  // Normalize graph data
  const normalizedGraph = useMemo((): GraphData | null => {
    if (!currentGraph) return null
    return normalizeGraphData(currentGraph)
  }, [currentGraph])

  const handleNodeClick = (node: GraphNode) => {
    setSelectedNode(node.id === selectedNode?.id ? null : node)
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Graph Visualizer</h1>
        <p className="text-slate-400">Interactive rule relationship graphs with Node2Vec</p>
      </div>

      {/* Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <MetricCard title="View Mode" value={viewMode} subtitle="Current view" />
        <MetricCard title="Nodes" value={normalizedGraph?.nodes.length || 0} subtitle="In graph" />
        <MetricCard title="Links" value={normalizedGraph?.links.length || 0} subtitle="Connections" />
        <MetricCard
          title="Min Similarity"
          value={`${(minSimilarity * 100).toFixed(0)}%`}
          subtitle="Threshold"
        />
      </div>

      <div className="grid grid-cols-12 gap-6">
        {/* Controls */}
        <div className="col-span-3 space-y-4">
          {/* View Mode */}
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">View Mode</h2>
            <div className="space-y-2">
              {(['single', 'network', 'comparison'] as ViewMode[]).map((mode) => (
                <button
                  key={mode}
                  onClick={() => {
                    setViewMode(mode)
                    setSelectedNode(null)
                  }}
                  className={`w-full text-left p-3 rounded-lg transition-colors capitalize ${
                    viewMode === mode
                      ? 'bg-primary-600 text-white'
                      : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                  }`}
                >
                  {mode === 'single' && 'Single Rule'}
                  {mode === 'network' && 'Network View'}
                  {mode === 'comparison' && 'Rule Comparison'}
                </button>
              ))}
            </div>
          </div>

          {/* Rule Selection (for single view) */}
          {viewMode === 'single' && (
            <div className="card">
              <h2 className="text-lg font-semibold text-white mb-4">Select Rule</h2>
              {rulesLoading ? (
                <p className="text-slate-400">Loading...</p>
              ) : (
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
              )}
            </div>
          )}

          {/* Similarity Threshold (for network view) */}
          {viewMode === 'network' && (
            <div className="card">
              <h2 className="text-lg font-semibold text-white mb-4">Similarity Threshold</h2>
              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <span className="text-slate-400">Min Similarity</span>
                  <span className="text-slate-300">{(minSimilarity * 100).toFixed(0)}%</span>
                </div>
                <input
                  type="range"
                  min="0.5"
                  max="0.95"
                  step="0.05"
                  value={minSimilarity}
                  onChange={(e) => setMinSimilarity(parseFloat(e.target.value))}
                  className="w-full accent-primary-500"
                />
              </div>
            </div>
          )}

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
                  <option value="type">Node Type</option>
                </select>
              </div>
              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  id="showLabels"
                  checked={showLabels}
                  onChange={(e) => setShowLabels(e.target.checked)}
                  className="rounded border-slate-600 bg-slate-900 text-primary-600"
                />
                <label htmlFor="showLabels" className="text-sm text-slate-300">
                  Show Labels
                </label>
              </div>
            </div>
          </div>

          {/* Selected Node Info */}
          {selectedNode && (
            <div className="card">
              <h2 className="text-lg font-semibold text-white mb-4">Selected Node</h2>
              <div className="space-y-2 text-sm">
                <p className="text-slate-300">
                  <span className="text-slate-400">ID:</span> {selectedNode.id}
                </p>
                <p className="text-slate-300">
                  <span className="text-slate-400">Label:</span> {selectedNode.label}
                </p>
                <p className="text-slate-300">
                  <span className="text-slate-400">Type:</span> {selectedNode.type}
                </p>
                {selectedNode.jurisdiction && (
                  <p className="text-slate-300">
                    <span className="text-slate-400">Jurisdiction:</span> {selectedNode.jurisdiction}
                  </p>
                )}
                {selectedNode.cluster !== undefined && (
                  <p className="text-slate-300">
                    <span className="text-slate-400">Cluster:</span> {selectedNode.cluster}
                  </p>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Graph Visualization */}
        <div className="col-span-9">
          <div className="card min-h-[600px] flex flex-col">
            <h2 className="text-lg font-semibold text-white mb-4">Force-Directed Graph</h2>

            {isLoading ? (
              <div className="flex-1 flex items-center justify-center">
                <LoadingOverlay message="Loading graph..." />
              </div>
            ) : error ? (
              <ErrorMessage message="Failed to load graph" />
            ) : !normalizedGraph || normalizedGraph.nodes.length === 0 ? (
              <div className="flex-1 flex items-center justify-center text-slate-400">
                {viewMode === 'single' && !selectedRuleId
                  ? 'Select a rule to view its graph'
                  : 'No graph data available'}
              </div>
            ) : (
              <div className="flex-1">
                <ForceGraph
                  data={normalizedGraph}
                  width={800}
                  height={550}
                  colorBy={colorBy}
                  showLabels={showLabels}
                  minLinkWeight={minSimilarity}
                  onNodeClick={handleNodeClick}
                />
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}

// Helper function to normalize graph data from API
function normalizeGraphData(data: unknown): GraphData {
  if (!data || typeof data !== 'object') {
    return { nodes: [], links: [] }
  }

  const obj = data as Record<string, unknown>

  // Handle different possible formats
  const nodes = (obj.nodes || []) as Array<Record<string, unknown>>
  const links = (obj.links || obj.edges || []) as Array<Record<string, unknown>>

  const normalizedNodes = nodes.map((node) => ({
    id: String(node.id || node.node_id || node.rule_id),
    label: String(node.label || node.name || node.rule_id || node.id),
    type: (node.type as 'rule' | 'condition' | 'outcome' | 'obligation') || 'rule',
    jurisdiction: node.jurisdiction as string | undefined,
    cluster: node.cluster as number | undefined,
    x: node.x as number | undefined,
    y: node.y as number | undefined,
  }))

  const normalizedLinks = links.map((link) => ({
    source: String(link.source || link.from),
    target: String(link.target || link.to),
    type: (link.type as 'implies' | 'requires' | 'conflicts' | 'similar' | 'child') || 'similar',
    weight: typeof link.weight === 'number' ? link.weight : (link.similarity as number) || 0.5,
  }))

  return {
    nodes: normalizedNodes,
    links: normalizedLinks,
  }
}
