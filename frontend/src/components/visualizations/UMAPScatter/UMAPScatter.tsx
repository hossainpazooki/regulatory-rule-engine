import { useRef, useEffect, useState, useCallback } from 'react'
import * as d3 from 'd3'
import type { UMAPPoint } from '@/types'
import { JURISDICTION_COLORS, CLUSTER_COLORS } from '@/types'

interface UMAPScatterProps {
  data: UMAPPoint[]
  width?: number
  height?: number
  colorBy?: 'jurisdiction' | 'cluster'
  selectedPoint?: string | null
  highlightedCluster?: number | null
  onPointClick?: (point: UMAPPoint) => void
  onPointHover?: (point: UMAPPoint | null) => void
  onBrushEnd?: (points: UMAPPoint[]) => void
}

export function UMAPScatter({
  data,
  width = 800,
  height = 600,
  colorBy = 'jurisdiction',
  selectedPoint,
  highlightedCluster,
  onPointClick,
  onPointHover,
  onBrushEnd,
}: UMAPScatterProps) {
  const svgRef = useRef<SVGSVGElement>(null)
  const [tooltipContent, setTooltipContent] = useState<string | null>(null)
  const [tooltipPosition, setTooltipPosition] = useState({ x: 0, y: 0 })

  const getPointColor = useCallback(
    (point: UMAPPoint): string => {
      if (colorBy === 'jurisdiction') {
        return JURISDICTION_COLORS[point.jurisdiction || 'default'] || JURISDICTION_COLORS.default
      } else {
        return CLUSTER_COLORS[point.cluster_id || 0] || CLUSTER_COLORS[0]
      }
    },
    [colorBy]
  )

  useEffect(() => {
    if (!svgRef.current || !data.length) return

    const svg = d3.select(svgRef.current)
    svg.selectAll('*').remove()

    const margin = { top: 40, right: 40, bottom: 60, left: 60 }
    const innerWidth = width - margin.left - margin.right
    const innerHeight = height - margin.top - margin.bottom

    // Create scales
    const xExtent = d3.extent(data, (d) => d.x) as [number, number]
    const yExtent = d3.extent(data, (d) => d.y) as [number, number]

    const xScale = d3.scaleLinear()
      .domain([xExtent[0] - 0.5, xExtent[1] + 0.5])
      .range([0, innerWidth])

    const yScale = d3.scaleLinear()
      .domain([yExtent[0] - 0.5, yExtent[1] + 0.5])
      .range([innerHeight, 0])

    // Create zoom behavior
    const zoom = d3.zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.5, 20])
      .extent([[0, 0], [width, height]])
      .on('zoom', zoomed)

    svg.call(zoom)

    // Create container group
    const g = svg.append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`)

    // Add clip path
    svg.append('defs').append('clipPath')
      .attr('id', 'scatter-clip')
      .append('rect')
      .attr('width', innerWidth)
      .attr('height', innerHeight)

    // Axes
    const xAxis = d3.axisBottom(xScale).ticks(10)
    const yAxis = d3.axisLeft(yScale).ticks(10)

    const xAxisG = g.append('g')
      .attr('class', 'x-axis')
      .attr('transform', `translate(0,${innerHeight})`)
      .call(xAxis)

    xAxisG.selectAll('text').attr('fill', '#94a3b8')
    xAxisG.selectAll('line').attr('stroke', '#475569')
    xAxisG.select('.domain').attr('stroke', '#475569')

    const yAxisG = g.append('g')
      .attr('class', 'y-axis')
      .call(yAxis)

    yAxisG.selectAll('text').attr('fill', '#94a3b8')
    yAxisG.selectAll('line').attr('stroke', '#475569')
    yAxisG.select('.domain').attr('stroke', '#475569')

    // Axis labels
    g.append('text')
      .attr('x', innerWidth / 2)
      .attr('y', innerHeight + 45)
      .attr('text-anchor', 'middle')
      .attr('fill', '#94a3b8')
      .attr('font-size', '12px')
      .text('UMAP Dimension 1')

    g.append('text')
      .attr('transform', 'rotate(-90)')
      .attr('x', -innerHeight / 2)
      .attr('y', -45)
      .attr('text-anchor', 'middle')
      .attr('fill', '#94a3b8')
      .attr('font-size', '12px')
      .text('UMAP Dimension 2')

    // Create points container
    const pointsG = g.append('g')
      .attr('class', 'points')
      .attr('clip-path', 'url(#scatter-clip)')

    // Draw points
    const points = pointsG.selectAll('circle')
      .data(data)
      .enter()
      .append('circle')
      .attr('cx', (d) => xScale(d.x))
      .attr('cy', (d) => yScale(d.y))
      .attr('r', (d) => d.rule_id === selectedPoint ? 8 : 5)
      .attr('fill', (d) => getPointColor(d))
      .attr('stroke', (d) => d.rule_id === selectedPoint ? '#ffffff' : '#1e293b')
      .attr('stroke-width', (d) => d.rule_id === selectedPoint ? 2 : 1)
      .attr('opacity', (d) => {
        if (highlightedCluster !== null && highlightedCluster !== undefined) {
          return d.cluster_id === highlightedCluster ? 1 : 0.15
        }
        return 0.8
      })
      .style('cursor', 'pointer')
      .on('click', (event, d) => {
        event.stopPropagation()
        onPointClick?.(d)
      })
      .on('mouseenter', (event, d) => {
        onPointHover?.(d)
        setTooltipContent(`${d.rule_id}${d.rule_name ? '\n' + d.rule_name : ''}${d.jurisdiction ? '\nJurisdiction: ' + d.jurisdiction : ''}${d.cluster_id !== undefined ? '\nCluster: ' + d.cluster_id : ''}`)
        setTooltipPosition({ x: event.pageX, y: event.pageY })
      })
      .on('mousemove', (event) => {
        setTooltipPosition({ x: event.pageX, y: event.pageY })
      })
      .on('mouseleave', () => {
        onPointHover?.(null)
        setTooltipContent(null)
      })

    // Brush for selection
    if (onBrushEnd) {
      const brush = d3.brush<unknown>()
        .extent([[0, 0], [innerWidth, innerHeight]])
        .on('end', (event) => {
          if (!event.selection) return
          const [[x0, y0], [x1, y1]] = event.selection as [[number, number], [number, number]]
          const selected = data.filter((d) => {
            const px = xScale(d.x)
            const py = yScale(d.y)
            return px >= x0 && px <= x1 && py >= y0 && py <= y1
          })
          onBrushEnd(selected)
          // Clear brush
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          g.select('.brush').call(brush.move as any, null)
        })

      g.append('g')
        .attr('class', 'brush')
        .call(brush)
    }

    // Zoom function
    function zoomed(event: d3.D3ZoomEvent<SVGSVGElement, unknown>) {
      const newXScale = event.transform.rescaleX(xScale)
      const newYScale = event.transform.rescaleY(yScale)

      xAxisG.call(xAxis.scale(newXScale))
      yAxisG.call(yAxis.scale(newYScale))

      xAxisG.selectAll('text').attr('fill', '#94a3b8')
      xAxisG.selectAll('line').attr('stroke', '#475569')
      xAxisG.select('.domain').attr('stroke', '#475569')

      yAxisG.selectAll('text').attr('fill', '#94a3b8')
      yAxisG.selectAll('line').attr('stroke', '#475569')
      yAxisG.select('.domain').attr('stroke', '#475569')

      points
        .attr('cx', (d) => newXScale(d.x))
        .attr('cy', (d) => newYScale(d.y))
    }

    // Double-click to reset zoom
    svg.on('dblclick.zoom', () => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      svg.transition().duration(500).call(zoom.transform as any, d3.zoomIdentity)
    })
  }, [data, width, height, colorBy, selectedPoint, highlightedCluster, getPointColor, onPointClick, onPointHover, onBrushEnd])

  // Get unique values for legend
  const legendItems = colorBy === 'jurisdiction'
    ? [...new Set(data.map((d) => d.jurisdiction).filter(Boolean))]
    : [...new Set(data.map((d) => d.cluster_id).filter((c) => c !== undefined))]

  return (
    <div className="relative w-full h-full">
      <svg
        ref={svgRef}
        width={width}
        height={height}
        className="bg-slate-900 rounded-lg"
      />

      {/* Tooltip */}
      {tooltipContent && (
        <div
          className="fixed z-50 px-3 py-2 bg-slate-800 border border-slate-600 rounded-lg shadow-lg text-sm text-slate-200 whitespace-pre-line pointer-events-none"
          style={{
            left: tooltipPosition.x + 15,
            top: tooltipPosition.y - 10,
          }}
        >
          {tooltipContent}
        </div>
      )}

      {/* Legend */}
      <div className="absolute top-4 right-4 flex flex-col gap-1 text-xs text-slate-400 bg-slate-800/80 p-3 rounded max-h-[200px] overflow-y-auto">
        <div className="font-medium text-slate-300 mb-1 capitalize">{colorBy}</div>
        {legendItems.slice(0, 10).map((item) => (
          <div key={String(item)} className="flex items-center gap-2">
            <span
              className="w-3 h-3 rounded-full"
              style={{
                backgroundColor:
                  colorBy === 'jurisdiction'
                    ? JURISDICTION_COLORS[item as string] || JURISDICTION_COLORS.default
                    : CLUSTER_COLORS[item as number] || CLUSTER_COLORS[0],
              }}
            />
            <span>{colorBy === 'cluster' ? `Cluster ${item}` : item}</span>
          </div>
        ))}
        {legendItems.length > 10 && (
          <div className="text-slate-500">+{legendItems.length - 10} more</div>
        )}
      </div>

      <div className="absolute bottom-4 left-4 text-xs text-slate-400">
        {data.length} points | Scroll to zoom | Double-click to reset
      </div>
    </div>
  )
}
