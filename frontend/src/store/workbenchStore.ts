import { create } from 'zustand'
import type { RuleInfo, TraceStep, DecisionResponse } from '@/types'

interface WorkbenchState {
  // Selected rule
  selectedRule: RuleInfo | null
  setSelectedRule: (rule: RuleInfo | null) => void

  // Trace state
  trace: TraceStep[]
  setTrace: (trace: TraceStep[]) => void
  highlightedNodes: string[]
  setHighlightedNodes: (nodes: string[]) => void

  // Decision state
  lastDecision: DecisionResponse | null
  setLastDecision: (decision: DecisionResponse | null) => void

  // UI state
  sidebarOpen: boolean
  toggleSidebar: () => void
  activeTab: 'trace' | 'test' | 'context' | 'review'
  setActiveTab: (tab: 'trace' | 'test' | 'context' | 'review') => void

  // Worklist
  worklist: RuleInfo[]
  addToWorklist: (rule: RuleInfo) => void
  removeFromWorklist: (ruleId: string) => void
  clearWorklist: () => void
}

export const useWorkbenchStore = create<WorkbenchState>((set) => ({
  // Selected rule
  selectedRule: null,
  setSelectedRule: (rule) => set({ selectedRule: rule }),

  // Trace state
  trace: [],
  setTrace: (trace) => set({ trace }),
  highlightedNodes: [],
  setHighlightedNodes: (nodes) => set({ highlightedNodes: nodes }),

  // Decision state
  lastDecision: null,
  setLastDecision: (decision) => set({ lastDecision: decision }),

  // UI state
  sidebarOpen: true,
  toggleSidebar: () => set((state) => ({ sidebarOpen: !state.sidebarOpen })),
  activeTab: 'trace',
  setActiveTab: (tab) => set({ activeTab: tab }),

  // Worklist
  worklist: [],
  addToWorklist: (rule) =>
    set((state) => ({
      worklist: state.worklist.some((r) => r.rule_id === rule.rule_id)
        ? state.worklist
        : [...state.worklist, rule],
    })),
  removeFromWorklist: (ruleId) =>
    set((state) => ({
      worklist: state.worklist.filter((r) => r.rule_id !== ruleId),
    })),
  clearWorklist: () => set({ worklist: [] }),
}))
