import { NavLink } from 'react-router-dom'
import {
  HomeIcon,
  DocumentTextIcon,
  BeakerIcon,
  GlobeAltIcon,
  CubeTransparentIcon,
  MagnifyingGlassIcon,
  ShareIcon,
  ChartBarIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  DocumentArrowUpIcon,
} from '@heroicons/react/24/outline'
import { useWorkbenchStore } from '@/store'

const navItems = [
  { to: '/', icon: HomeIcon, label: 'Home' },
  { to: '/workbench', icon: DocumentTextIcon, label: 'KE Workbench' },
  { to: '/production', icon: BeakerIcon, label: 'Production Demo' },
  { to: '/navigate', icon: GlobeAltIcon, label: 'Cross-Border' },
  { to: '/embeddings', icon: CubeTransparentIcon, label: 'Embeddings' },
  { to: '/similarity', icon: MagnifyingGlassIcon, label: 'Similarity' },
  { to: '/graph', icon: ShareIcon, label: 'Graph' },
  { to: '/analytics', icon: ChartBarIcon, label: 'Analytics' },
  { to: '/documents', icon: DocumentArrowUpIcon, label: 'Document Ingestion' },
]

export function Sidebar() {
  const { sidebarOpen, toggleSidebar } = useWorkbenchStore()

  return (
    <aside
      className={`fixed left-0 top-0 h-full bg-slate-800 border-r border-slate-700 transition-all duration-300 z-50 ${
        sidebarOpen ? 'w-64' : 'w-16'
      }`}
    >
      {/* Header */}
      <div className="flex items-center justify-between h-16 px-4 border-b border-slate-700">
        {sidebarOpen && (
          <span className="text-lg font-semibold text-white truncate">Regulatory KE</span>
        )}
        <button
          onClick={toggleSidebar}
          className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-700 transition-colors"
        >
          {sidebarOpen ? (
            <ChevronLeftIcon className="w-5 h-5" />
          ) : (
            <ChevronRightIcon className="w-5 h-5" />
          )}
        </button>
      </div>

      {/* Navigation */}
      <nav className="p-2 space-y-1">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            className={({ isActive }) =>
              `flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors ${
                isActive
                  ? 'bg-primary-600 text-white'
                  : 'text-slate-300 hover:bg-slate-700 hover:text-white'
              }`
            }
          >
            <item.icon className="w-5 h-5 flex-shrink-0" />
            {sidebarOpen && <span className="truncate">{item.label}</span>}
          </NavLink>
        ))}
      </nav>

      {/* Footer */}
      {sidebarOpen && (
        <div className="absolute bottom-0 left-0 right-0 p-4 border-t border-slate-700">
          <div className="text-xs text-slate-500">
            <p>Regulatory KE Workbench</p>
            <p>v0.1.0</p>
          </div>
        </div>
      )}
    </aside>
  )
}
