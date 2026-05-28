import { useRef, useEffect, useCallback } from 'react'
import * as d3 from 'd3'
import type { TreeNode } from '@/types'

interface DecisionTreeProps {
  data: TreeNode
  width?: number
  height?: number
  highlightedPath?: string[]
  onNodeClick?: (node: TreeNode) => void
}

const NODE_COLORS = {
  condition: {
    consistent: '#22c55e',
    inconsistent: '#ef4444',
    unknown: '#6b7280',
  },
  outcome: {
    consistent: '#3b82f6',
    inconsistent: '#f97316',
    unknown: '#8b5cf6',
  },
}

export function DecisionTree({
  data,
  width = 900,
  height = 600,
  highlightedPath = [],
  onNodeClick,
}: DecisionTreeProps) {
  const svgRef = useRef<SVGSVGElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  const getNodeColor = useCallback((node: TreeNode): string => {
    const colorSet = NODE_COLORS[node.type]
    return colorSet[node.consistency || 'unknown']
  }, [])

  const isHighlighted = useCallback(
    (nodeId: string): boolean => {
      return highlightedPath.includes(nodeId)
    },
    [highlightedPath]
  )

  useEffect(() => {
    if (!svgRef.current || !data) return

    const svg = d3.select(svgRef.current)
    svg.selectAll('*').remove()

    const margin = { top: 40, right: 120, bottom: 40, left: 120 }
    const innerWidth = width - margin.left - margin.right
    const innerHeight = height - margin.top - margin.bottom

    // Create zoom behavior
    const zoom = d3.zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.3, 3])
      .on('zoom', (event) => {
        g.attr('transform', event.transform)
      })

    svg.call(zoom)

    // Add arrow marker for links
    svg.append('defs').append('marker')
      .attr('id', 'arrow')
      .attr('viewBox', '0 -5 10 10')
      .attr('refX', 15)
      .attr('refY', 0)
      .attr('markerWidth', 6)
      .attr('markerHeight', 6)
      .attr('orient', 'auto')
      .append('path')
      .attr('fill', '#64748b')
      .attr('d', 'M0,-5L10,0L0,5')

    const g = svg.append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`)

    // Create tree layout
    const treeLayout = d3.tree<TreeNode>()
      .size([innerHeight, innerWidth])
      .separation((a, b) => (a.parent === b.parent ? 1.5 : 2))

    // Create hierarchy
    const root = d3.hierarchy(data)
    const treeData = treeLayout(root)

    // Draw links
    g.selectAll('.link')
      .data(treeData.links())
      .enter()
      .append('path')
      .attr('class', 'link')
      .attr('fill', 'none')
      .attr('stroke', (d) => {
        const sourceHighlighted = isHighlighted(d.source.data.id)
        const targetHighlighted = isHighlighted(d.target.data.id)
        return sourceHighlighted && targetHighlighted ? '#f59e0b' : '#475569'
      })
      .attr('stroke-width', (d) => {
        const sourceHighlighted = isHighlighted(d.source.data.id)
        const targetHighlighted = isHighlighted(d.target.data.id)
        return sourceHighlighted && targetHighlighted ? 3 : 1.5
      })
      .attr('d', d3.linkHorizontal<d3.HierarchyPointLink<TreeNode>, d3.HierarchyPointNode<TreeNode>>()
        .x((d) => d.y)
        .y((d) => d.x)
      )

    // Draw nodes
    const nodes = g.selectAll('.node')
      .data(treeData.descendants())
      .enter()
      .append('g')
      .attr('class', 'node')
      .attr('transform', (d) => `translate(${d.y},${d.x})`)
      .style('cursor', 'pointer')
      .on('click', (event, d) => {
        event.stopPropagation()
        onNodeClick?.(d.data)
      })

    // Node circles
    nodes.append('circle')
      .attr('r', (d) => d.data.type === 'outcome' ? 12 : 10)
      .attr('fill', (d) => getNodeColor(d.data))
      .attr('stroke', (d) => isHighlighted(d.data.id) ? '#f59e0b' : '#1e293b')
      .attr('stroke-width', (d) => isHighlighted(d.data.id) ? 3 : 2)
      .attr('opacity', (d) => isHighlighted(d.data.id) || highlightedPath.length === 0 ? 1 : 0.5)

    // Node labels
    nodes.append('text')
      .attr('dy', '0.35em')
      .attr('x', (d) => d.children ? -16 : 16)
      .attr('text-anchor', (d) => d.children ? 'end' : 'start')
      .attr('font-size', '12px')
      .attr('fill', (d) => isHighlighted(d.data.id) || highlightedPath.length === 0 ? '#f8fafc' : '#94a3b8')
      .attr('font-weight', (d) => isHighlighted(d.data.id) ? 'bold' : 'normal')
      .text((d) => truncateLabel(d.data.label, 30))

    // Tooltips
    nodes.append('title')
      .text((d) => {
        const node = d.data
        let tooltip = `${node.label}\nType: ${node.type}`
        if (node.condition) tooltip += `\nCondition: ${node.condition}`
        if (node.result) tooltip += `\nResult: ${node.result}`
        if (node.consistency) tooltip += `\nStatus: ${node.consistency}`
        return tooltip
      })

    // Initial zoom to fit
    const bounds = g.node()?.getBBox()
    if (bounds) {
      const dx = bounds.width
      const dy = bounds.height
      const x = bounds.x + dx / 2
      const y = bounds.y + dy / 2
      const scale = 0.9 / Math.max(dx / innerWidth, dy / innerHeight)
      const translate: [number, number] = [innerWidth / 2 - scale * x + margin.left, innerHeight / 2 - scale * y + margin.top]

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      svg.call(zoom.transform as any, d3.zoomIdentity.translate(translate[0], translate[1]).scale(scale))
    }
  }, [data, width, height, highlightedPath, getNodeColor, isHighlighted, onNodeClick])

  return (
    <div ref={containerRef} className="relative w-full h-full">
      <svg
        ref={svgRef}
        width={width}
        height={height}
        className="bg-slate-900 rounded-lg"
      />
      <div className="absolute bottom-4 right-4 flex gap-4 text-xs text-slate-400">
        <div className="flex items-center gap-2">
          <span className="w-3 h-3 rounded-full bg-green-500" />
          <span>Consistent</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-3 h-3 rounded-full bg-red-500" />
          <span>Inconsistent</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-3 h-3 rounded-full bg-gray-500" />
          <span>Unknown</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-3 h-3 rounded-full border-2 border-amber-500" />
          <span>Trace Path</span>
        </div>
      </div>
    </div>
  )
}

function truncateLabel(label: string, maxLength: number): string {
  if (label.length <= maxLength) return label
  return label.slice(0, maxLength - 3) + '...'
}
