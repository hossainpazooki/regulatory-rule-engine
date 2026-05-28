import { Routes, Route } from 'react-router-dom'
import { Layout } from './components/common/Layout'
import { Home } from './pages/Home'
import { KEWorkbench } from './pages/KEWorkbench'
import { ProductionDemo } from './pages/ProductionDemo'
import { CrossBorderNavigator } from './pages/CrossBorderNavigator'
import { EmbeddingExplorer } from './pages/EmbeddingExplorer'
import { SimilaritySearch } from './pages/SimilaritySearch'
import { GraphVisualizer } from './pages/GraphVisualizer'
import { AnalyticsDashboard } from './pages/AnalyticsDashboard'
import { DocumentIngestion } from './pages/DocumentIngestion'

export default function App() {
  return (
    <Layout>
      <Routes>
        <Route path="/" element={<Home />} />
        <Route path="/workbench" element={<KEWorkbench />} />
        <Route path="/production" element={<ProductionDemo />} />
        <Route path="/navigate" element={<CrossBorderNavigator />} />
        <Route path="/embeddings" element={<EmbeddingExplorer />} />
        <Route path="/similarity" element={<SimilaritySearch />} />
        <Route path="/graph" element={<GraphVisualizer />} />
        <Route path="/analytics" element={<AnalyticsDashboard />} />
        <Route path="/documents" element={<DocumentIngestion />} />
      </Routes>
    </Layout>
  )
}
