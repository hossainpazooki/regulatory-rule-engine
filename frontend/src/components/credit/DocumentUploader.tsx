import { useState, useCallback, DragEvent } from 'react'
import { ArrowUpTrayIcon, DocumentIcon } from '@heroicons/react/24/outline'

interface DocumentUploaderProps {
  onUpload: (file: { filename: string; content_type: string; raw_text: string }) => void
  isLoading: boolean
}

export function DocumentUploader({ onUpload, isLoading }: DocumentUploaderProps) {
  const [dragOver, setDragOver] = useState(false)
  const [selectedFile, setSelectedFile] = useState<File | null>(null)

  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault()
    setDragOver(true)
  }, [])

  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault()
    setDragOver(false)
  }, [])

  const handleDrop = useCallback((e: DragEvent) => {
    e.preventDefault()
    setDragOver(false)
    const file = e.dataTransfer.files[0]
    if (file) setSelectedFile(file)
  }, [])

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (file) setSelectedFile(file)
  }

  const handleUpload = async () => {
    if (!selectedFile) return
    const text = await selectedFile.text()
    onUpload({
      filename: selectedFile.name,
      content_type: selectedFile.type || 'application/octet-stream',
      raw_text: text,
    })
    setSelectedFile(null)
  }

  return (
    <div className="space-y-4">
      <div
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        className={`border-2 border-dashed rounded-lg p-8 text-center transition-colors cursor-pointer ${
          dragOver
            ? 'border-primary-400 bg-primary-900/20'
            : 'border-slate-600 hover:border-slate-500'
        }`}
        onClick={() => document.getElementById('file-input')?.click()}
      >
        <ArrowUpTrayIcon className="w-10 h-10 mx-auto text-slate-400 mb-3" />
        <p className="text-slate-300 text-sm">Drag and drop a document here, or click to browse</p>
        <p className="text-slate-500 text-xs mt-1">PDF, TXT, DOCX supported</p>
        <input
          id="file-input"
          type="file"
          className="hidden"
          onChange={handleFileChange}
          accept=".pdf,.txt,.docx,.doc,.csv"
        />
      </div>

      {selectedFile && (
        <div className="flex items-center justify-between p-3 bg-slate-700 rounded-lg">
          <div className="flex items-center gap-2">
            <DocumentIcon className="w-5 h-5 text-slate-400" />
            <span className="text-sm text-slate-200 truncate">{selectedFile.name}</span>
            <span className="text-xs text-slate-500">{(selectedFile.size / 1024).toFixed(1)} KB</span>
          </div>
          <button
            onClick={handleUpload}
            disabled={isLoading}
            className="btn-primary text-sm px-4 py-1.5"
          >
            {isLoading ? 'Uploading...' : 'Upload'}
          </button>
        </div>
      )}
    </div>
  )
}
