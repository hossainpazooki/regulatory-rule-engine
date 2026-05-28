import type { ClassificationResult } from '@/api/credit.api'

interface ClassificationCardProps {
  result: ClassificationResult
}

function confidenceColor(confidence: number): string {
  if (confidence > 0.8) return 'bg-green-500'
  if (confidence > 0.6) return 'bg-yellow-500'
  return 'bg-red-500'
}

function typeBadgeColor(type: string): string {
  const colors: Record<string, string> = {
    CIM: 'bg-blue-600',
    'Financial Statement': 'bg-emerald-600',
    'Legal Opinion': 'bg-purple-600',
    'Term Sheet': 'bg-amber-600',
    'Credit Agreement': 'bg-cyan-600',
  }
  return colors[type] || 'bg-slate-600'
}

export function ClassificationCard({ result }: ClassificationCardProps) {
  const fields = Object.entries(result.extracted_fields)

  return (
    <div className="card">
      <div className="flex items-center justify-between mb-3">
        <span
          className={`px-2.5 py-1 rounded text-xs font-medium text-white ${typeBadgeColor(result.predicted_type)}`}
        >
          {result.predicted_type}
        </span>
        <span className="text-xs text-slate-400">{result.document_id}</span>
      </div>

      <div className="mb-3">
        <div className="flex items-center justify-between text-xs text-slate-400 mb-1">
          <span>Confidence</span>
          <span>{(result.confidence * 100).toFixed(0)}%</span>
        </div>
        <div className="w-full h-2 bg-slate-700 rounded-full">
          <div
            className={`h-full rounded-full transition-all ${confidenceColor(result.confidence)}`}
            style={{ width: `${result.confidence * 100}%` }}
          />
        </div>
      </div>

      {fields.length > 0 && (
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-slate-400 uppercase tracking-wider">
            Extracted Fields
          </p>
          {fields.map(([key, value]) => (
            <div key={key} className="flex justify-between text-sm">
              <span className="text-slate-400">{key}</span>
              <span className="text-slate-200 truncate ml-4">{String(value)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
