import { useRef, useEffect, useState, useCallback } from 'react'
import * as d3 from 'd3'
import type { GraphNode, GraphLink, GraphData } from '@/types'
import { JURISDICTION_COLORS, CLUSTER_COLORS } from '@/types'

interface ForceGraphProps {
  data: GraphData
  width?: number
  height?: number
  colorBy?: 'jurisdiction' | 'cluster' | 'type'
  showLabels?: boolean
  minLinkWeight?: number
  onNodeClick?: (node: GraphNode) => void
  onNodeHover?: (node: GraphNode | null) => void
}

type SimulationNode = GraphNode & d3.SimulationNodeDatum
type SimulationLink = d3.SimulationLinkDatum<SimulationNode> & {
  type: GraphLink['type']
  weight?: number
}

const LINK_COLORS: Record<GraphLink['type'], string> = {
  implies: '#22c55e',
  requires: '#3b82f6',
  conflicts: '#ef4444',
  similar: '#8b5cf6',
  child: '#64748b',
}

export function ForceGraph({
  data,
  width = 800,
  height = 600,
  colorBy = 'jurisdiction',
  showLabels = true,
  minLinkWeight = 0,
  onNodeClick,
  onNodeHover,
}: ForceGraphProps) {
  const svgRef = useRef<SVGSVGElement>(null)
  const [selectedNode, setSelectedNode] = useState<string | null>(null)

  const getNodeColor = useCallback(
    (node: GraphNode): string => {
      switch (colorBy) {
        case 'jurisdiction':
          return JURISDICTION_COLORS[node.jurisdiction || 'default'] || JURISDICTION_COLORS.default
        case 'cluster':
          return CLUSTER_COLORS[node.cluster || 0] || CLUSTER_COLORS[0]
        case 'type':
          return node.type === 'rule'
            ? '#3b82f6'
            : node.type === 'condition'
              ? '#22c55e'
              : node.type === 'outcome'
                ? '#f59e0b'
                : '#8b5cf6'
        default:
          return '#6b7280'
      }
    },
    [colorBy]
  )

  useEffect(() => {
    if (!svgRef.current || !data.nodes.length) return

    const svg = d3.select(svgRef.current)
    svg.selectAll('*').remove()

    // Filter links by minimum weight
    const filteredLinks: SimulationLink[] = data.links
      .filter((l) => (l.weight || 1) >= minLinkWeight)
      .map((l) => ({
        source: typeof l.source === 'string' ? l.source : l.source.id,
        target: typeof l.target === 'string' ? l.target : l.target.id,
        type: l.type,
        weight: l.weight,
      }))

    const nodes: SimulationNode[] = data.nodes.map((n) => ({ ...n }))

    // Create zoom behavior
    const zoom = d3.zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.2, 5])
      .on('zoom', (event) => {
        container.attr('transform', event.transform)
      })

    svg.call(zoom)

    // Add arrow markers
    const defs = svg.append('defs')
    Object.entries(LINK_COLORS).forEach(([type, color]) => {
      defs.append('marker')
        .attr('id', `arrow-${type}`)
        .attr('viewBox', '0 -5 10 10')
        .attr('refX', 20)
        .attr('refY', 0)
        .attr('markerWidth', 6)
        .attr('markerHeight', 6)
        .attr('orient', 'auto')
        .append('path')
        .attr('fill', color)
        .attr('d', 'M0,-5L10,0L0,5')
    })

    const container = svg.append('g')

    // Create force simulation
    const simulation = d3.forceSimulation<SimulationNode>(nodes)
      .force('link', d3.forceLink<SimulationNode, SimulationLink>(filteredLinks)
        .id((d) => d.id)
        .distance((d) => 100 - (d.weight || 0.5) * 50)
        .strength((d) => (d.weight || 0.5) * 0.5)
      )
      .force('charge', d3.forceManyBody().strength(-200))
      .force('center', d3.forceCenter(width / 2, height / 2))
      .force('collision', d3.forceCollide().radius(30))

    // Draw links
    const link = container.append('g')
      .attr('class', 'links')
      .selectAll('line')
      .data(filteredLinks)
      .enter()
      .append('line')
      .attr('stroke', (d) => LINK_COLORS[d.type])
      .attr('stroke-width', (d) => Math.max(1, (d.weight || 0.5) * 3))
      .attr('stroke-opacity', 0.6)
      .attr('marker-end', (d) => d.type !== 'similar' ? `url(#arrow-${d.type})` : null)

    // Draw nodes
    const node = container.append('g')
      .attr('class', 'nodes')
      .selectAll('g')
      .data(nodes)
      .enter()
      .append('g')
      .style('cursor', 'pointer')
      .call(d3.drag<SVGGElement, SimulationNode>()
        .on('start', dragstarted)
        .on('drag', dragged)
        .on('end', dragended)
      )
      .on('click', (event, d) => {
        event.stopPropagation()
        setSelectedNode(d.id === selectedNode ? null : d.id)
        onNodeClick?.(d)
      })
      .on('mouseenter', (_event, d) => {
        onNodeHover?.(d)
        highlightConnections(d.id)
      })
      .on('mouseleave', () => {
        onNodeHover?.(null)
        resetHighlight()
      })

    // Node circles
    node.append('circle')
      .attr('r', (d) => d.type === 'rule' ? 12 : 8)
      .attr('fill', (d) => getNodeColor(d))
      .attr('stroke', '#1e293b')
      .attr('stroke-width', 2)

    // Node labels
    if (showLabels) {
      node.append('text')
        .attr('dy', -16)
        .attr('text-anchor', 'middle')
        .attr('font-size', '10px')
        .attr('fill', '#e2e8f0')
        .attr('pointer-events', 'none')
        .text((d) => truncateLabel(d.label, 20))
    }

    // Tooltips
    node.append('title')
      .text((d) => `${d.label}\nType: ${d.type}${d.jurisdiction ? `\nJurisdiction: ${d.jurisdiction}` : ''}`)

    // Highlight functions
    function highlightConnections(nodeId: string) {
      const connectedIds = new Set<string>()
      connectedIds.add(nodeId)

      filteredLinks.forEach((l) => {
        const sourceId = String(typeof l.source === 'object' ? l.source.id : l.source)
        const targetId = String(typeof l.target === 'object' ? l.target.id : l.target)
        if (sourceId === nodeId) connectedIds.add(targetId)
        if (targetId === nodeId) connectedIds.add(sourceId)
      })

      node.select('circle')
        .attr('opacity', (d) => connectedIds.has(d.id) ? 1 : 0.2)

      link.attr('stroke-opacity', (d) => {
        const sourceId = typeof d.source === 'object' ? d.source.id : d.source
        const targetId = typeof d.target === 'object' ? d.target.id : d.target
        return sourceId === nodeId || targetId === nodeId ? 0.8 : 0.1
      })
    }

    function resetHighlight() {
      node.select('circle').attr('opacity', 1)
      link.attr('stroke-opacity', 0.6)
    }

    // Drag functions
    function dragstarted(event: d3.D3DragEvent<SVGGElement, SimulationNode, SimulationNode>) {
      if (!event.active) simulation.alphaTarget(0.3).restart()
      event.subject.fx = event.subject.x
      event.subject.fy = event.subject.y
    }

    function dragged(event: d3.D3DragEvent<SVGGElement, SimulationNode, SimulationNode>) {
      event.subject.fx = event.x
      event.subject.fy = event.y
    }

    function dragended(event: d3.D3DragEvent<SVGGElement, SimulationNode, SimulationNode>) {
      if (!event.active) simulation.alphaTarget(0)
      event.subject.fx = null
      event.subject.fy = null
    }

    // Update positions on tick
    simulation.on('tick', () => {
      link
        .attr('x1', (d) => (d.source as SimulationNode).x || 0)
        .attr('y1', (d) => (d.source as SimulationNode).y || 0)
        .attr('x2', (d) => (d.target as SimulationNode).x || 0)
        .attr('y2', (d) => (d.target as SimulationNode).y || 0)

      node.attr('transform', (d) => `translate(${d.x || 0},${d.y || 0})`)
    })

    // Cleanup
    return () => {
      simulation.stop()
    }
  }, [data, width, height, colorBy, showLabels, minLinkWeight, getNodeColor, onNodeClick, onNodeHover, selectedNode])

  return (
    <div className="relative w-full h-full">
      <svg
        ref={svgRef}
        width={width}
        height={height}
        className="bg-slate-900 rounded-lg"
      />
      <div className="absolute bottom-4 left-4 flex flex-col gap-2 text-xs text-slate-400 bg-slate-800/80 p-3 rounded">
        <div className="font-medium text-slate-300 mb-1">Link Types</div>
        {Object.entries(LINK_COLORS).map(([type, color]) => (
          <div key={type} className="flex items-center gap-2">
            <span className="w-4 h-0.5" style={{ backgroundColor: color }} />
            <span className="capitalize">{type}</span>
          </div>
        ))}
      </div>
      <div className="absolute top-4 right-4 text-xs text-slate-400">
        Drag to move nodes | Scroll to zoom | Click to select
      </div>
    </div>
  )
}

function truncateLabel(label: string, maxLength: number): string {
  if (label.length <= maxLength) return label
  return label.slice(0, maxLength - 3) + '...'
}
