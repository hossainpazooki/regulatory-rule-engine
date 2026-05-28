type Status = 'success' | 'warning' | 'error' | 'info' | 'neutral'

interface StatusBadgeProps {
  status: Status
  label: string
  size?: 'sm' | 'md'
}

const statusStyles: Record<Status, string> = {
  success: 'bg-green-500/20 text-green-400 border-green-500/30',
  warning: 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30',
  error: 'bg-red-500/20 text-red-400 border-red-500/30',
  info: 'bg-blue-500/20 text-blue-400 border-blue-500/30',
  neutral: 'bg-slate-500/20 text-slate-400 border-slate-500/30',
}

export function StatusBadge({ status, label, size = 'md' }: StatusBadgeProps) {
  return (
    <span
      className={`inline-flex items-center border rounded-full font-medium ${statusStyles[status]} ${
        size === 'sm' ? 'px-2 py-0.5 text-xs' : 'px-3 py-1 text-sm'
      }`}
    >
      {label}
    </span>
  )
}
