import { create } from 'zustand'
import type { EmbeddingType, UMAPPoint } from '@/types'

interface AnalyticsState {
  // Embedding explorer state
  selectedEmbeddingType: EmbeddingType
  setSelectedEmbeddingType: (type: EmbeddingType) => void

  // UMAP state
  selectedPoint: UMAPPoint | null
  setSelectedPoint: (point: UMAPPoint | null) => void
  hoveredPoint: UMAPPoint | null
  setHoveredPoint: (point: UMAPPoint | null) => void

  // Cluster state (cluster ID for highlighting)
  selectedCluster: number | null
  setSelectedCluster: (clusterId: number | null) => void

  // Filter state
  jurisdictionFilter: string[]
  setJurisdictionFilter: (jurisdictions: string[]) => void
  clusterFilter: number[]
  setClusterFilter: (clusters: number[]) => void

  // Similarity search state
  searchWeights: Record<EmbeddingType, number>
  setSearchWeight: (type: EmbeddingType, weight: number) => void
  resetSearchWeights: () => void
}

const DEFAULT_WEIGHTS: Record<EmbeddingType, number> = {
  semantic: 0.4,
  structural: 0.25,
  entity: 0.2,
  legal: 0.15,
  graph: 0,
  all: 1,
}

export const useAnalyticsStore = create<AnalyticsState>((set) => ({
  // Embedding explorer state
  selectedEmbeddingType: 'semantic',
  setSelectedEmbeddingType: (type) => set({ selectedEmbeddingType: type }),

  // UMAP state
  selectedPoint: null,
  setSelectedPoint: (point) => set({ selectedPoint: point }),
  hoveredPoint: null,
  setHoveredPoint: (point) => set({ hoveredPoint: point }),

  // Cluster state
  selectedCluster: null,
  setSelectedCluster: (cluster) => set({ selectedCluster: cluster }),

  // Filter state
  jurisdictionFilter: [],
  setJurisdictionFilter: (jurisdictions) => set({ jurisdictionFilter: jurisdictions }),
  clusterFilter: [],
  setClusterFilter: (clusters) => set({ clusterFilter: clusters }),

  // Similarity search state
  searchWeights: { ...DEFAULT_WEIGHTS },
  setSearchWeight: (type, weight) =>
    set((state) => ({
      searchWeights: { ...state.searchWeights, [type]: weight },
    })),
  resetSearchWeights: () => set({ searchWeights: { ...DEFAULT_WEIGHTS } }),
}))
