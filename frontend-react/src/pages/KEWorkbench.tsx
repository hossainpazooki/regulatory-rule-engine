import { useState, useMemo } from 'react'
import { useRules, useRule, useRuleTree, useDecision } from '@/hooks'
import { useWorkbenchStore } from '@/store'
import { LoadingOverlay, ErrorMessage, StatusBadge } from '@/components/common'
import { DecisionTree } from '@/components/visualizations'
import type { RuleInfo, DecideRequest, TreeNode } from '@/types'

// Mock legal analysis data
const MOCK_LEGAL_ANALYSIS = {
  regulatory_flags: [
    { flag: 'MiCA Article 16 - Reserve requirements not met', severity: 'high' },
    { flag: 'Cross-border passport notification pending', severity: 'medium' },
    { flag: 'ESMA reporting deadline approaching (30 days)', severity: 'low' },
  ],
  covenant_issues: [
    { issue: 'Debt-to-equity ratio exceeds 4:1 threshold', section: 'Section 7.2(a)' },
    { issue: 'Material adverse change clause triggered', section: 'Section 9.1(c)' },
    { issue: 'Information covenants - quarterly reporting overdue', section: 'Section 6.3' },
  ],
  jurisdiction_risks: [
    { jurisdiction: 'EU', risk: 'Pending MiCA transitional provisions expire Q3 2026', level: 'medium' },
    { jurisdiction: 'UK', risk: 'FCA crypto registration renewal required', level: 'high' },
    { jurisdiction: 'US', risk: 'State-by-state MSB licensing gaps in 3 states', level: 'medium' },
  ],
  citations: [
    { source: 'MiCA Regulation (EU) 2023/1114, Art. 16', text: 'Reserve of assets requirements for issuers of asset-referenced tokens' },
    { source: 'FCA PS22/10, Section 4.3', text: 'Cryptoasset registration and ongoing obligations' },
    { source: 'Basel III Framework, CRE 20.4', text: 'Credit risk exposure classification for digital assets' },
  ],
}

type WorkbenchTab = 'rules' | 'legal'

export function KEWorkbench() {
  const [activeTab, setActiveTab] = useState<WorkbenchTab>('rules')
  const { data: rulesData, isLoading: rulesLoading, error: rulesError } = useRules()
  const { selectedRule, setSelectedRule, trace, setTrace, setLastDecision, highlightedNodes, setHighlightedNodes } = useWorkbenchStore()
  const { data: ruleDetail } = useRule(selectedRule?.rule_id || '')
  const { data: treeData, isLoading: treeLoading } = useRuleTree(selectedRule?.rule_id || '')
  const decideMutation = useDecision()

  const [scenario, setScenario] = useState<DecideRequest>({
    instrument_type: 'art',
    activity: 'public_offer',
    jurisdiction: 'EU',
    authorized: false,
    is_credit_institution: false,
  })

  // Transform tree data to TreeNode format
  const normalizedTree = useMemo(() => {
    if (!treeData) return null
    return normalizeTreeData(treeData)
  }, [treeData])

  // Extract highlighted path from trace
  const highlightedPath = useMemo(() => {
    return trace.map((step) => step.node)
  }, [trace])

  const handleRunTrace = async () => {
    if (!selectedRule) return

    const result = await decideMutation.mutateAsync({
      ...scenario,
      rule_id: selectedRule.rule_id,
    })

    if (result.results.length > 0) {
      setTrace(result.results[0].trace)
      setLastDecision(result.results[0])
      // Highlight traced nodes
      const traceNodes = result.results[0].trace.map((step) => step.node)
      setHighlightedNodes(traceNodes)
    }
  }

  const handleNodeClick = (node: TreeNode) => {
    // Toggle highlight for clicked node
    if (highlightedNodes.includes(node.id)) {
      setHighlightedNodes(highlightedNodes.filter((id) => id !== node.id))
    } else {
      setHighlightedNodes([...highlightedNodes, node.id])
    }
  }

  if (rulesLoading) return <LoadingOverlay message="Loading rules..." />
  if (rulesError) return <ErrorMessage message="Failed to load rules" />

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-white">KE Workbench</h1>
        <p className="text-slate-400">Verify and review rules with decision tree visualization</p>
      </div>

      {/* Tab bar */}
      <div className="border-b border-slate-700">
        <nav className="flex gap-4">
          {([
            { key: 'rules' as const, label: 'Rules & Trace' },
            { key: 'legal' as const, label: 'Legal Analysis' },
          ]).map((tab) => (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              className={`py-3 border-b-2 font-medium transition-colors ${
                activeTab === tab.key
                  ? 'border-primary-500 text-primary-400'
                  : 'border-transparent text-slate-400 hover:text-white'
              }`}
            >
              {tab.label}
            </button>
          ))}
        </nav>
      </div>

      {/* Legal Analysis Tab */}
      {activeTab === 'legal' && (
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Regulatory Flags */}
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Regulatory Flags</h2>
            <div className="space-y-3">
              {MOCK_LEGAL_ANALYSIS.regulatory_flags.map((item, idx) => (
                <div key={idx} className="p-3 bg-slate-700 rounded-lg">
                  <div className="flex items-center gap-2 mb-1">
                    <StatusBadge
                      status={
                        item.severity === 'high' ? 'error' : item.severity === 'medium' ? 'warning' : 'info'
                      }
                      label={item.severity}
                      size="sm"
                    />
                  </div>
                  <p className="text-sm text-slate-200">{item.flag}</p>
                </div>
              ))}
            </div>
          </div>

          {/* Covenant Issues */}
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Covenant Issues</h2>
            <div className="space-y-3">
              {MOCK_LEGAL_ANALYSIS.covenant_issues.map((item, idx) => (
                <div key={idx} className="p-3 bg-slate-700 rounded-lg">
                  <p className="text-sm text-slate-200">{item.issue}</p>
                  <p className="text-xs text-slate-500 mt-1">{item.section}</p>
                </div>
              ))}
            </div>
          </div>

          {/* Jurisdiction Risks */}
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Jurisdiction Risks</h2>
            <div className="space-y-3">
              {MOCK_LEGAL_ANALYSIS.jurisdiction_risks.map((item, idx) => (
                <div key={idx} className="p-3 bg-slate-700 rounded-lg">
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-sm font-medium text-white">{item.jurisdiction}</span>
                    <StatusBadge
                      status={item.level === 'high' ? 'error' : item.level === 'medium' ? 'warning' : 'info'}
                      label={item.level}
                      size="sm"
                    />
                  </div>
                  <p className="text-sm text-slate-300">{item.risk}</p>
                </div>
              ))}
            </div>
          </div>

          {/* Citations */}
          <div className="card lg:col-span-3">
            <h2 className="text-lg font-semibold text-white mb-4">Citations</h2>
            <div className="space-y-3">
              {MOCK_LEGAL_ANALYSIS.citations.map((cite, idx) => (
                <div key={idx} className="p-3 bg-slate-700 rounded-lg">
                  <p className="text-sm font-medium text-primary-400">{cite.source}</p>
                  <p className="text-sm text-slate-300 mt-1">{cite.text}</p>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Tri-pane layout */}
      {activeTab === 'rules' && (
      <div className="grid grid-cols-12 gap-6">
        {/* Left Panel: Rule List */}
        <div className="col-span-3">
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Rules</h2>
            <div className="space-y-2 max-h-[600px] overflow-auto">
              {rulesData?.rules.map((rule: RuleInfo) => (
                <button
                  key={rule.rule_id}
                  onClick={() => {
                    setSelectedRule(rule)
                    setTrace([])
                    setHighlightedNodes([])
                  }}
                  className={`w-full text-left p-3 rounded-lg transition-colors ${
                    selectedRule?.rule_id === rule.rule_id
                      ? 'bg-primary-600 text-white'
                      : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                  }`}
                >
                  <p className="font-medium truncate">{rule.rule_id}</p>
                  <p className="text-sm opacity-75 truncate">{rule.description}</p>
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* Center Panel: Decision Tree */}
        <div className="col-span-6">
          <div className="card min-h-[600px] flex flex-col">
            <h2 className="text-lg font-semibold text-white mb-4">Decision Tree</h2>
            {selectedRule ? (
              treeLoading ? (
                <div className="flex-1 flex items-center justify-center">
                  <LoadingOverlay message="Loading decision tree..." />
                </div>
              ) : normalizedTree ? (
                <div className="flex-1">
                  <DecisionTree
                    data={normalizedTree}
                    width={600}
                    height={500}
                    highlightedPath={highlightedPath}
                    onNodeClick={handleNodeClick}
                  />
                </div>
              ) : (
                <div className="flex-1 flex flex-col items-center justify-center text-slate-400">
                  <p className="text-lg mb-2">No tree visualization available</p>
                  {ruleDetail?.decision_tree && (
                    <pre className="mt-4 text-left bg-slate-900 p-4 rounded-lg overflow-auto max-h-96 text-xs w-full">
                      {JSON.stringify(ruleDetail.decision_tree, null, 2)}
                    </pre>
                  )}
                </div>
              )
            ) : (
              <div className="flex-1 flex items-center justify-center text-slate-400">
                Select a rule to view its decision tree
              </div>
            )}
          </div>
        </div>

        {/* Right Panel: Controls & Trace */}
        <div className="col-span-3 space-y-4">
          {/* Scenario Form */}
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Test Scenario</h2>
            <div className="space-y-3">
              <div>
                <label className="block text-sm text-slate-400 mb-1">Instrument Type</label>
                <select
                  value={scenario.instrument_type || ''}
                  onChange={(e) => setScenario({ ...scenario, instrument_type: e.target.value })}
                  className="input w-full"
                >
                  <option value="art">Asset-Referenced Token</option>
                  <option value="emt">E-Money Token</option>
                  <option value="utility">Utility Token</option>
                  <option value="security">Security Token</option>
                </select>
              </div>
              <div>
                <label className="block text-sm text-slate-400 mb-1">Activity</label>
                <select
                  value={scenario.activity || ''}
                  onChange={(e) => setScenario({ ...scenario, activity: e.target.value })}
                  className="input w-full"
                >
                  <option value="public_offer">Public Offer</option>
                  <option value="admission_to_trading">Admission to Trading</option>
                  <option value="custody">Custody Services</option>
                  <option value="exchange">Exchange Services</option>
                </select>
              </div>
              <div>
                <label className="block text-sm text-slate-400 mb-1">Jurisdiction</label>
                <select
                  value={scenario.jurisdiction || ''}
                  onChange={(e) => setScenario({ ...scenario, jurisdiction: e.target.value })}
                  className="input w-full"
                >
                  <option value="EU">EU (MiCA)</option>
                  <option value="UK">UK (FCA)</option>
                  <option value="US">US (GENIUS)</option>
                  <option value="CH">Switzerland (FINMA)</option>
                  <option value="SG">Singapore (MAS)</option>
                </select>
              </div>
              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  id="authorized"
                  checked={scenario.authorized || false}
                  onChange={(e) => setScenario({ ...scenario, authorized: e.target.checked })}
                  className="rounded border-slate-600 bg-slate-900 text-primary-600"
                />
                <label htmlFor="authorized" className="text-sm text-slate-300">
                  Authorized entity
                </label>
              </div>
              <button
                onClick={handleRunTrace}
                disabled={!selectedRule || decideMutation.isPending}
                className="btn-primary w-full mt-4"
              >
                {decideMutation.isPending ? 'Running...' : 'Run Trace'}
              </button>
            </div>
          </div>

          {/* Trace Results */}
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Trace</h2>
            {trace.length > 0 ? (
              <div className="space-y-2">
                {trace.map((step, idx) => (
                  <div key={idx} className="flex items-start gap-2 text-sm">
                    <StatusBadge
                      status={step.result ? 'success' : 'error'}
                      label={step.result ? 'T' : 'F'}
                      size="sm"
                    />
                    <div>
                      <p className="text-slate-300">{step.condition}</p>
                      <p className="text-xs text-slate-500">Node: {step.node}</p>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-slate-400 text-sm">Run a trace to see results</p>
            )}
          </div>
        </div>
      </div>
      )}
    </div>
  )
}

// Helper function to normalize tree data from API to TreeNode format
function normalizeTreeData(data: unknown): TreeNode | null {
  if (!data || typeof data !== 'object') return null

  const obj = data as Record<string, unknown>

  // Handle ChartDataResponse format from backend (has data property)
  if ('data' in obj && obj.data && typeof obj.data === 'object') {
    return normalizeTreeNode(obj.data)
  }

  // Handle different possible tree formats
  if ('tree' in obj && obj.tree) {
    return normalizeTreeNode(obj.tree)
  }

  if ('nodes' in obj && Array.isArray(obj.nodes)) {
    // Convert flat nodes to hierarchical
    return buildTreeFromNodes(obj.nodes as Array<Record<string, unknown>>)
  }

  // Try direct conversion
  return normalizeTreeNode(data)
}

function normalizeTreeNode(node: unknown): TreeNode | null {
  if (!node || typeof node !== 'object') return null

  const obj = node as Record<string, unknown>

  // Map backend type to frontend type
  const nodeType = obj.type === 'leaf' ? 'outcome' :
                   obj.type === 'branch' ? 'condition' :
                   (obj.type as 'condition' | 'outcome') || (obj.children ? 'condition' : 'outcome')

  // Build label from title (backend) or other fields
  let label = String(obj.title || obj.label || obj.name || obj.id || 'Node')

  // For branch nodes, show condition if available
  if (obj.type === 'branch' && obj.condition) {
    label = String(obj.condition)
  }

  const treeNode: TreeNode = {
    id: String(obj.id || obj.node_id || obj.title || Math.random().toString(36)),
    label,
    type: nodeType,
    condition: obj.condition as string | undefined,
    result: obj.result as string | undefined,
    consistency: obj.consistency as 'consistent' | 'inconsistent' | 'unknown' | undefined,
    isTracePath: obj.isTracePath as boolean | undefined,
  }

  // Handle children (backend uses branch: "true"/"false" to indicate path)
  if (obj.children && Array.isArray(obj.children)) {
    treeNode.children = obj.children
      .map((child) => {
        const childObj = child as Record<string, unknown>
        const childNode = normalizeTreeNode(child)
        if (childNode && childObj.branch) {
          // Prefix label with branch direction
          const prefix = childObj.branch === 'true' ? 'Yes' : 'No'
          childNode.label = `${prefix}: ${childNode.label}`
        }
        return childNode
      })
      .filter((child): child is TreeNode => child !== null)
  }

  // Handle yes/no branches (alternative format)
  if (obj.yes || obj.no) {
    treeNode.children = []
    if (obj.yes) {
      const yesNode = normalizeTreeNode(obj.yes)
      if (yesNode) {
        yesNode.label = `Yes: ${yesNode.label}`
        treeNode.children.push(yesNode)
      }
    }
    if (obj.no) {
      const noNode = normalizeTreeNode(obj.no)
      if (noNode) {
        noNode.label = `No: ${noNode.label}`
        treeNode.children.push(noNode)
      }
    }
  }

  return treeNode
}

function buildTreeFromNodes(nodes: Array<Record<string, unknown>>): TreeNode | null {
  if (nodes.length === 0) return null

  // Find root node (no parent)
  const nodeMap = new Map<string, Record<string, unknown>>()
  const childIds = new Set<string>()

  nodes.forEach((node) => {
    const id = String(node.id || node.node_id)
    nodeMap.set(id, node)

    // Track child IDs
    if (node.children && Array.isArray(node.children)) {
      (node.children as string[]).forEach((childId) => childIds.add(String(childId)))
    }
  })

  // Find root (not a child of any other node)
  let rootNode: Record<string, unknown> | undefined
  for (const node of nodes) {
    const id = String(node.id || node.node_id)
    if (!childIds.has(id)) {
      rootNode = node
      break
    }
  }

  if (!rootNode) {
    rootNode = nodes[0]
  }

  // Build tree recursively
  function buildNode(node: Record<string, unknown>): TreeNode {
    const treeNode: TreeNode = {
      id: String(node.id || node.node_id || Math.random().toString(36)),
      label: String(node.label || node.name || node.condition || 'Node'),
      type: (node.type as 'condition' | 'outcome') || 'condition',
      condition: node.condition as string | undefined,
      result: node.result as string | undefined,
      consistency: node.consistency as 'consistent' | 'inconsistent' | 'unknown' | undefined,
    }

    if (node.children && Array.isArray(node.children)) {
      treeNode.children = (node.children as string[])
        .map((childId) => {
          const childNode = nodeMap.get(String(childId))
          return childNode ? buildNode(childNode) : null
        })
        .filter((child): child is TreeNode => child !== null)
    }

    return treeNode
  }

  return buildNode(rootNode)
}
