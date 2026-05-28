interface FeatureToggleProps {
  label: string
  enabled?: boolean
  detail?: string
}

export function FeatureToggle({ label, enabled, detail }: FeatureToggleProps) {
  return (
    <div className="flex items-center justify-between p-3 bg-slate-600/50 rounded">
      <div>
        <span className="text-sm text-white">{label}</span>
        {detail && <p className="text-xs text-slate-400">{detail}</p>}
      </div>
      <span
        className={`w-2 h-2 rounded-full ${enabled ? 'bg-green-500' : 'bg-slate-500'}`}
      />
    </div>
  )
}
