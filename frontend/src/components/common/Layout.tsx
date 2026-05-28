import { ReactNode } from 'react'
import { Sidebar } from './Sidebar'
import { useWorkbenchStore } from '@/store'

interface LayoutProps {
  children: ReactNode
}

export function Layout({ children }: LayoutProps) {
  const sidebarOpen = useWorkbenchStore((state) => state.sidebarOpen)

  return (
    <div className="flex h-screen bg-slate-900">
      <Sidebar />
      <main
        className={`flex-1 overflow-auto transition-all duration-300 ${
          sidebarOpen ? 'ml-64' : 'ml-16'
        }`}
      >
        <div className="p-6">{children}</div>
      </main>
    </div>
  )
}
