import { useState } from 'react'
import { MetricCard } from '@/components/common'
import { DocumentUploader } from '@/components/credit/DocumentUploader'
import { ClassificationCard } from '@/components/credit/ClassificationCard'
import { useUploadDocument } from '@/hooks'
import type { ClassificationResult } from '@/api/credit.api'
import {
  DocumentTextIcon,
  CheckCircleIcon,
  ChartBarIcon,
} from '@heroicons/react/24/outline'

// Mock data for demo rendering without backend
const MOCK_CLASSIFICATIONS: ClassificationResult[] = [
  {
    document_id: 'doc-001',
    predicted_type: 'CIM',
    confidence: 0.94,
    extracted_fields: {
      issuer: 'Acme Corp',
      deal_size: '$250M',
      sector: 'Technology',
      date: '2026-02-15',
    },
  },
  {
    document_id: 'doc-002',
    predicted_type: 'Financial Statement',
    confidence: 0.87,
    extracted_fields: {
      entity: 'Acme Corp',
      period: 'FY2025',
      revenue: '$1.2B',
      ebitda: '$340M',
    },
  },
  {
    document_id: 'doc-003',
    predicted_type: 'Legal Opinion',
    confidence: 0.62,
    extracted_fields: {
      firm: 'Baker McKenzie',
      jurisdiction: 'Delaware',
      opinion_type: 'Enforceability',
    },
  },
]

const PIPELINE_PHASES = ['upload', 'classify', 'index', 'ready']

interface UploadedDoc {
  filename: string
  document_id: string
  status: 'uploaded' | 'classified' | 'indexed' | 'ready'
}

export function DocumentIngestion() {
  const [uploadedDocs, setUploadedDocs] = useState<UploadedDoc[]>([
    { filename: 'acme_cim_2026.pdf', document_id: 'doc-001', status: 'ready' },
    { filename: 'acme_financials_fy25.xlsx', document_id: 'doc-002', status: 'classified' },
    { filename: 'legal_opinion_delaware.pdf', document_id: 'doc-003', status: 'classified' },
  ])
  const [classifications, setClassifications] =
    useState<ClassificationResult[]>(MOCK_CLASSIFICATIONS)

  const uploadMutation = useUploadDocument()

  const currentPhaseIndex = Math.min(
    ...uploadedDocs.map((d) => PIPELINE_PHASES.indexOf(d.status)),
    PIPELINE_PHASES.length - 1
  )

  const avgConfidence =
    classifications.length > 0
      ? classifications.reduce((sum, c) => sum + c.confidence, 0) / classifications.length
      : 0

  const handleUpload = (file: { filename: string; content_type: string; raw_text: string }) => {
    uploadMutation.mutate(file, {
      onSuccess: (response) => {
        const result = response.data
        const docId = result.document_id || `doc-${Date.now()}`
        setUploadedDocs((prev) => [
          ...prev,
          { filename: file.filename, document_id: docId, status: 'classified' },
        ])
        setClassifications((prev) => [...prev, result])
      },
      onError: () => {
        // Add as uploaded even if classification fails
        const docId = `doc-${Date.now()}`
        setUploadedDocs((prev) => [
          ...prev,
          { filename: file.filename, document_id: docId, status: 'uploaded' },
        ])
      },
    })
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Document Ingestion</h1>
        <p className="text-slate-400">
          Upload, classify, and index credit documents for analysis
        </p>
      </div>

      {/* Pipeline Progress */}
      <div className="card">
        <p className="text-sm text-slate-400 mb-3">Pipeline Progress</p>
        <div className="flex items-center gap-2">
          {PIPELINE_PHASES.map((phase, idx) => (
            <div key={phase} className="flex items-center flex-1">
              <div className="flex flex-col items-center flex-1">
                <div
                  className={`w-8 h-8 rounded-full flex items-center justify-center text-xs font-medium ${
                    idx <= currentPhaseIndex
                      ? 'bg-primary-600 text-white'
                      : 'bg-slate-700 text-slate-400'
                  }`}
                >
                  {idx + 1}
                </div>
                <span className="text-xs text-slate-400 mt-1 capitalize">{phase}</span>
              </div>
              {idx < PIPELINE_PHASES.length - 1 && (
                <div
                  className={`h-0.5 flex-1 -mt-4 ${
                    idx < currentPhaseIndex ? 'bg-primary-500' : 'bg-slate-700'
                  }`}
                />
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <MetricCard
          title="Documents Uploaded"
          value={uploadedDocs.length}
          icon={<DocumentTextIcon className="w-6 h-6" />}
        />
        <MetricCard
          title="Classified"
          value={classifications.length}
          icon={<CheckCircleIcon className="w-6 h-6" />}
        />
        <MetricCard
          title="Avg Confidence"
          value={`${(avgConfidence * 100).toFixed(0)}%`}
          icon={<ChartBarIcon className="w-6 h-6" />}
        />
      </div>

      {/* Two-column layout */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Left: Upload + Document List */}
        <div className="space-y-4">
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Upload Document</h2>
            <DocumentUploader onUpload={handleUpload} isLoading={uploadMutation.isPending} />
          </div>

          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Uploaded Documents</h2>
            {uploadedDocs.length === 0 ? (
              <p className="text-slate-400 text-sm">No documents uploaded yet</p>
            ) : (
              <div className="space-y-2">
                {uploadedDocs.map((doc) => (
                  <div
                    key={doc.document_id}
                    className="flex items-center justify-between p-3 bg-slate-700 rounded-lg"
                  >
                    <div className="flex items-center gap-2">
                      <DocumentTextIcon className="w-4 h-4 text-slate-400" />
                      <span className="text-sm text-slate-200 truncate">{doc.filename}</span>
                    </div>
                    <span
                      className={`text-xs px-2 py-0.5 rounded capitalize ${
                        doc.status === 'ready'
                          ? 'bg-green-900/50 text-green-400'
                          : doc.status === 'classified'
                            ? 'bg-blue-900/50 text-blue-400'
                            : doc.status === 'indexed'
                              ? 'bg-yellow-900/50 text-yellow-400'
                              : 'bg-slate-600 text-slate-300'
                      }`}
                    >
                      {doc.status}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Right: Classification Results */}
        <div className="space-y-4">
          <h2 className="text-lg font-semibold text-white">Classification Results</h2>
          {classifications.length === 0 ? (
            <div className="card">
              <p className="text-slate-400 text-sm">
                Upload documents to see classification results
              </p>
            </div>
          ) : (
            classifications.map((result) => (
              <ClassificationCard key={result.document_id} result={result} />
            ))
          )}
        </div>
      </div>
    </div>
  )
}
