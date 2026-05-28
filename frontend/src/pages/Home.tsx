import { Link } from 'react-router-dom'
import {
  DocumentTextIcon,
  BeakerIcon,
  GlobeAltIcon,
  CubeTransparentIcon,
  MagnifyingGlassIcon,
  ShareIcon,
  ChartBarIcon,
} from '@heroicons/react/24/outline'
import { MetricCard } from '@/components/common'
import { useRules } from '@/hooks'

const navigationCards = [
  {
    to: '/workbench',
    icon: DocumentTextIcon,
    title: 'KE Workbench',
    description: 'Verify and review rules with decision trees and trace analysis',
    color: 'bg-blue-500',
  },
  {
    to: '/production',
    icon: BeakerIcon,
    title: 'Production Demo',
    description: 'Synthetic scenarios, guardrails, and performance metrics',
    color: 'bg-purple-500',
  },
  {
    to: '/navigate',
    icon: GlobeAltIcon,
    title: 'Cross-Border Navigator',
    description: 'Multi-jurisdiction compliance pathways and conflict detection',
    color: 'bg-green-500',
  },
  {
    to: '/embeddings',
    icon: CubeTransparentIcon,
    title: 'Embedding Explorer',
    description: 'UMAP visualization of rule embeddings by type',
    color: 'bg-orange-500',
  },
  {
    to: '/similarity',
    icon: MagnifyingGlassIcon,
    title: 'Similarity Search',
    description: 'Find related rules across jurisdictions with weighted search',
    color: 'bg-pink-500',
  },
  {
    to: '/graph',
    icon: ShareIcon,
    title: 'Graph Visualizer',
    description: 'Interactive rule relationship graphs with Node2Vec',
    color: 'bg-cyan-500',
  },
  {
    to: '/analytics',
    icon: ChartBarIcon,
    title: 'Analytics Dashboard',
    description: 'Clustering, coverage gaps, and conflict resolution',
    color: 'bg-indigo-500',
  },
]

const frameworks = [
  { name: 'MiCA', jurisdiction: 'EU', rules: 8, accuracy: 'High', status: 'Enacted' },
  { name: 'FCA Crypto', jurisdiction: 'UK', rules: 5, accuracy: 'High', status: 'Enacted' },
  { name: 'GENIUS Act', jurisdiction: 'US', rules: 6, accuracy: 'High', status: 'Enacted' },
  { name: 'SEC Securities', jurisdiction: 'US_SEC', rules: 0, accuracy: 'High', status: 'Enacted' },
  { name: 'CFTC Digital', jurisdiction: 'US_CFTC', rules: 0, accuracy: 'High', status: 'Enacted' },
  { name: 'FINMA DLT', jurisdiction: 'CH', rules: 6, accuracy: 'High', status: 'Enacted' },
  { name: 'MAS PSA', jurisdiction: 'SG', rules: 6, accuracy: 'High', status: 'Enacted' },
  { name: 'SFC VASP', jurisdiction: 'HK', rules: 0, accuracy: 'High', status: 'Enacted' },
  { name: 'PSA Japan', jurisdiction: 'JP', rules: 0, accuracy: 'High', status: 'Enacted' },
  { name: 'RWA Token', jurisdiction: 'EU', rules: 3, accuracy: 'Low', status: 'Hypothetical' },
]

export function Home() {
  const { data: rulesData, isLoading } = useRules()

  const totalRules = rulesData?.total || 0
  const jurisdictions = [...new Set(frameworks.map((f) => f.jurisdiction))].length

  return (
    <div className="space-y-8">
      {/* Header */}
      <div>
        <h1 className="text-3xl font-bold text-white">Regulatory KE Workbench</h1>
        <p className="text-slate-400 mt-2">
          Computational law platform for tokenized real-world assets
        </p>
      </div>

      {/* Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <MetricCard
          title="Total Rules"
          value={isLoading ? '...' : totalRules}
          subtitle="Across all frameworks"
        />
        <MetricCard title="Jurisdictions" value={jurisdictions} subtitle="Active regulatory zones" />
        <MetricCard title="Frameworks" value={frameworks.length} subtitle="Legal frameworks" />
        <MetricCard
          title="Embedding Types"
          value={4}
          subtitle="Semantic, structural, entity, legal"
        />
      </div>

      {/* Navigation Cards */}
      <div>
        <h2 className="text-xl font-semibold text-white mb-4">Features</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {navigationCards.map((card) => (
            <Link
              key={card.to}
              to={card.to}
              className="card hover:border-slate-600 transition-colors group"
            >
              <div className="flex items-start gap-4">
                <div className={`p-2 rounded-lg ${card.color}`}>
                  <card.icon className="w-6 h-6 text-white" />
                </div>
                <div>
                  <h3 className="font-semibold text-white group-hover:text-primary-400 transition-colors">
                    {card.title}
                  </h3>
                  <p className="text-sm text-slate-400 mt-1">{card.description}</p>
                </div>
              </div>
            </Link>
          ))}
        </div>
      </div>

      {/* Framework Table */}
      <div>
        <h2 className="text-xl font-semibold text-white mb-4">Regulatory Frameworks</h2>
        <div className="card overflow-hidden p-0">
          <table className="w-full">
            <thead>
              <tr className="border-b border-slate-700">
                <th className="text-left p-4 text-sm font-medium text-slate-400">Framework</th>
                <th className="text-left p-4 text-sm font-medium text-slate-400">Jurisdiction</th>
                <th className="text-left p-4 text-sm font-medium text-slate-400">Rules</th>
                <th className="text-left p-4 text-sm font-medium text-slate-400">Accuracy</th>
                <th className="text-left p-4 text-sm font-medium text-slate-400">Status</th>
              </tr>
            </thead>
            <tbody>
              {frameworks.map((framework) => (
                <tr
                  key={framework.name}
                  className="border-b border-slate-700/50 hover:bg-slate-700/30"
                >
                  <td className="p-4 text-white font-medium">{framework.name}</td>
                  <td className="p-4 text-slate-300">{framework.jurisdiction}</td>
                  <td className="p-4 text-slate-300">{framework.rules}</td>
                  <td className="p-4">
                    <span
                      className={`px-2 py-0.5 rounded text-xs font-medium ${
                        framework.accuracy === 'High'
                          ? 'bg-green-500/20 text-green-400'
                          : framework.accuracy === 'Medium'
                            ? 'bg-yellow-500/20 text-yellow-400'
                            : 'bg-red-500/20 text-red-400'
                      }`}
                    >
                      {framework.accuracy}
                    </span>
                  </td>
                  <td className="p-4">
                    <span
                      className={`px-2 py-0.5 rounded text-xs font-medium ${
                        framework.status === 'Enacted'
                          ? 'bg-blue-500/20 text-blue-400'
                          : framework.status === 'Proposed'
                            ? 'bg-yellow-500/20 text-yellow-400'
                            : 'bg-slate-500/20 text-slate-400'
                      }`}
                    >
                      {framework.status}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  )
}
