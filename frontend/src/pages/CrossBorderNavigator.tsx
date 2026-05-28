import { useState } from 'react'
import { MetricCard, LoadingOverlay, ErrorMessage, StatusBadge } from '@/components/common'
import { useJurisdictions, useNavigate } from '@/hooks'
import type { NavigateRequest, NavigateResponse } from '@/api/jurisdiction.api'

const instrumentTypes = [
  { value: 'e-money-token', label: 'E-Money Token (EMT)' },
  { value: 'asset-referenced-token', label: 'Asset-Referenced Token (ART)' },
  { value: 'utility-token', label: 'Utility Token' },
  { value: 'crypto-asset', label: 'Other Crypto-Asset' },
  { value: 'stablecoin', label: 'Stablecoin' },
  { value: 'tokenized_bond', label: 'Tokenized Bond' },
  { value: 'security_token', label: 'Security Token' },
]

const activities = [
  { value: 'public_offer', label: 'Public Offer' },
  { value: 'issuance', label: 'Token Issuance' },
  { value: 'custody', label: 'Custody Services' },
  { value: 'exchange', label: 'Exchange/Trading' },
  { value: 'transfer', label: 'Transfer Services' },
  { value: 'financial_promotion', label: 'Financial Promotion' },
  { value: 'advisory', label: 'Advisory Services' },
]

const investorTypes = [
  { value: 'retail', label: 'Retail Investors' },
  { value: 'professional', label: 'Professional Investors' },
  { value: 'qualified', label: 'Qualified Investors' },
]

const tokenStandards = [
  { value: '', label: 'Not Applicable' },
  { value: 'ERC-20', label: 'ERC-20 (Ethereum)' },
  { value: 'BEP-20', label: 'BEP-20 (BSC)' },
  { value: 'SPL', label: 'SPL (Solana)' },
  { value: 'TRC-20', label: 'TRC-20 (Tron)' },
]

const blockchains = [
  { value: '', label: 'Not Specified' },
  { value: 'ethereum', label: 'Ethereum' },
  { value: 'polygon', label: 'Polygon' },
  { value: 'solana', label: 'Solana' },
  { value: 'avalanche', label: 'Avalanche' },
]

const defiProtocols = [
  { value: '', label: 'None' },
  { value: 'aave_v3', label: 'Aave V3' },
  { value: 'uniswap_v3', label: 'Uniswap V3' },
  { value: 'lido', label: 'Lido' },
  { value: 'gmx', label: 'GMX' },
]

type Tab = 'pathway' | 'obligations' | 'conflicts' | 'risk'

export function CrossBorderNavigator() {
  const [activeTab, setActiveTab] = useState<Tab>('pathway')
  const [issuerJurisdiction, setIssuerJurisdiction] = useState('CH')
  const [targetJurisdictions, setTargetJurisdictions] = useState<string[]>(['EU', 'UK'])
  const [selectedInvestorTypes, setSelectedInvestorTypes] = useState<string[]>(['professional'])
  const [formData, setFormData] = useState({
    instrument_type: 'e-money-token',
    activity: 'public_offer',
    token_standard: '',
    underlying_chain: '',
    is_defi_integrated: false,
    defi_protocol: '',
  })
  const [result, setResult] = useState<NavigateResponse | null>(null)

  const { data: jurisdictions, isLoading: jurisdictionsLoading } = useJurisdictions()
  const { mutate: navigate, isPending: isNavigating, error: navigateError } = useNavigate()

  const toggleTargetJurisdiction = (code: string) => {
    if (code === issuerJurisdiction) return // Can't be both issuer and target
    setTargetJurisdictions((prev) =>
      prev.includes(code) ? prev.filter((j) => j !== code) : [...prev, code]
    )
  }

  const toggleInvestorType = (type: string) => {
    setSelectedInvestorTypes((prev) =>
      prev.includes(type) ? prev.filter((t) => t !== type) : [...prev, type]
    )
  }

  const handleAnalyze = () => {
    const request: NavigateRequest = {
      issuer_jurisdiction: issuerJurisdiction,
      target_jurisdictions: targetJurisdictions,
      instrument_type: formData.instrument_type,
      activity: formData.activity,
      investor_types: selectedInvestorTypes,
      facts: {},
      token_standard: formData.token_standard || undefined,
      underlying_chain: formData.underlying_chain || undefined,
      is_defi_integrated: formData.is_defi_integrated,
      defi_protocol: formData.is_defi_integrated ? formData.defi_protocol || undefined : undefined,
    }
    navigate(request, {
      onSuccess: (data) => setResult(data),
    })
  }

  // Fallback jurisdictions if API not available
  const displayJurisdictions = jurisdictions || [
    { code: 'EU', name: 'European Union', authority: 'ESMA' },
    { code: 'UK', name: 'United Kingdom', authority: 'FCA' },
    { code: 'US', name: 'United States', authority: 'SEC/CFTC' },
    { code: 'CH', name: 'Switzerland', authority: 'FINMA' },
    { code: 'SG', name: 'Singapore', authority: 'MAS' },
    { code: 'HK', name: 'Hong Kong', authority: 'SFC' },
    { code: 'JP', name: 'Japan', authority: 'FSA' },
  ]

  const totalObligations = result?.cumulative_obligations?.length || 0
  const conflictCount = result?.conflicts?.length || 0

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Cross-Border Navigator</h1>
        <p className="text-slate-400">Multi-jurisdiction compliance pathways with market risk analysis</p>
      </div>

      {/* Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-5 gap-4">
        <MetricCard title="Status" value={result?.status || '—'} subtitle="Overall" />
        <MetricCard title="Jurisdictions" value={result?.applicable_jurisdictions?.length || 0} subtitle="Applicable" />
        <MetricCard title="Obligations" value={totalObligations} subtitle="Cumulative" />
        <MetricCard title="Conflicts" value={conflictCount} subtitle="Detected" />
        <MetricCard title="Timeline" value={result?.estimated_timeline || '—'} subtitle="Estimated" />
      </div>

      <div className="grid grid-cols-12 gap-6">
        {/* Left Panel: Jurisdiction Selection + Scenario Builder */}
        <div className="col-span-4 space-y-4">
          {/* Issuer Jurisdiction */}
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Issuer Jurisdiction</h2>
            {jurisdictionsLoading ? (
              <LoadingOverlay message="Loading..." />
            ) : (
              <select
                value={issuerJurisdiction}
                onChange={(e) => setIssuerJurisdiction(e.target.value)}
                className="input w-full"
              >
                {displayJurisdictions.map((j) => (
                  <option key={j.code} value={j.code}>
                    {j.name} ({j.code})
                  </option>
                ))}
              </select>
            )}
          </div>

          {/* Target Jurisdictions */}
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Target Markets</h2>
            <div className="space-y-2 max-h-[200px] overflow-y-auto">
              {displayJurisdictions
                .filter((j) => j.code !== issuerJurisdiction)
                .map((j) => (
                  <button
                    key={j.code}
                    onClick={() => toggleTargetJurisdiction(j.code)}
                    className={`w-full text-left p-3 rounded-lg transition-colors ${
                      targetJurisdictions.includes(j.code)
                        ? 'bg-primary-600 text-white'
                        : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <span className="font-medium">{j.name}</span>
                      <span className="text-sm opacity-75">{j.authority}</span>
                    </div>
                  </button>
                ))}
            </div>
          </div>

          {/* Scenario Builder */}
          <div className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Scenario</h2>
            <div className="space-y-4">
              <div>
                <label className="block text-sm text-slate-400 mb-2">Instrument Type</label>
                <select
                  value={formData.instrument_type}
                  onChange={(e) => setFormData((prev) => ({ ...prev, instrument_type: e.target.value }))}
                  className="input w-full"
                >
                  {instrumentTypes.map((type) => (
                    <option key={type.value} value={type.value}>
                      {type.label}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-sm text-slate-400 mb-2">Activity</label>
                <select
                  value={formData.activity}
                  onChange={(e) => setFormData((prev) => ({ ...prev, activity: e.target.value }))}
                  className="input w-full"
                >
                  {activities.map((act) => (
                    <option key={act.value} value={act.value}>
                      {act.label}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-sm text-slate-400 mb-2">Investor Types</label>
                <div className="flex flex-wrap gap-2">
                  {investorTypes.map((type) => (
                    <button
                      key={type.value}
                      onClick={() => toggleInvestorType(type.value)}
                      className={`px-3 py-1 rounded-full text-sm ${
                        selectedInvestorTypes.includes(type.value)
                          ? 'bg-primary-600 text-white'
                          : 'bg-slate-700 text-slate-300'
                      }`}
                    >
                      {type.label}
                    </button>
                  ))}
                </div>
              </div>

              <div>
                <label className="block text-sm text-slate-400 mb-2">Token Standard</label>
                <select
                  value={formData.token_standard}
                  onChange={(e) => setFormData((prev) => ({ ...prev, token_standard: e.target.value }))}
                  className="input w-full"
                >
                  {tokenStandards.map((std) => (
                    <option key={std.value} value={std.value}>
                      {std.label}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-sm text-slate-400 mb-2">Blockchain</label>
                <select
                  value={formData.underlying_chain}
                  onChange={(e) => setFormData((prev) => ({ ...prev, underlying_chain: e.target.value }))}
                  className="input w-full"
                >
                  {blockchains.map((chain) => (
                    <option key={chain.value} value={chain.value}>
                      {chain.label}
                    </option>
                  ))}
                </select>
              </div>

              <div className="flex items-center gap-3">
                <input
                  type="checkbox"
                  id="isDefiIntegrated"
                  checked={formData.is_defi_integrated}
                  onChange={(e) => setFormData((prev) => ({ ...prev, is_defi_integrated: e.target.checked }))}
                  className="w-4 h-4 rounded border-slate-600 bg-slate-700 text-primary-500"
                />
                <label htmlFor="isDefiIntegrated" className="text-sm text-slate-300">
                  DeFi Integration
                </label>
              </div>

              {formData.is_defi_integrated && (
                <div>
                  <label className="block text-sm text-slate-400 mb-2">DeFi Protocol</label>
                  <select
                    value={formData.defi_protocol}
                    onChange={(e) => setFormData((prev) => ({ ...prev, defi_protocol: e.target.value }))}
                    className="input w-full"
                  >
                    {defiProtocols.map((proto) => (
                      <option key={proto.value} value={proto.value}>
                        {proto.label}
                      </option>
                    ))}
                  </select>
                </div>
              )}

              <button
                onClick={handleAnalyze}
                disabled={targetJurisdictions.length === 0 || isNavigating}
                className="btn-primary w-full"
              >
                {isNavigating ? 'Analyzing...' : 'Analyze Pathway'}
              </button>
            </div>
          </div>
        </div>

        {/* Right Panel: Results */}
        <div className="col-span-8">
          <div className="card min-h-[600px]">
            {/* Tabs */}
            <div className="border-b border-slate-700 -mx-6 -mt-6 px-6 mb-6">
              <nav className="flex gap-4">
                {(['pathway', 'obligations', 'conflicts', 'risk'] as Tab[]).map((tab) => (
                  <button
                    key={tab}
                    onClick={() => setActiveTab(tab)}
                    className={`py-4 border-b-2 font-medium transition-colors capitalize ${
                      activeTab === tab
                        ? 'border-primary-500 text-primary-400'
                        : 'border-transparent text-slate-400 hover:text-white'
                    }`}
                  >
                    {tab}
                    {tab === 'conflicts' && conflictCount > 0 && (
                      <span className="ml-2 px-1.5 py-0.5 bg-red-500/20 text-red-400 rounded text-xs">
                        {conflictCount}
                      </span>
                    )}
                  </button>
                ))}
              </nav>
            </div>

            {/* Content */}
            {isNavigating ? (
              <LoadingOverlay message="Analyzing compliance pathway..." />
            ) : navigateError ? (
              <ErrorMessage message="Failed to analyze pathway. Check backend connection." />
            ) : !result ? (
              <div className="text-center py-12 text-slate-400">
                <p>Select target markets and configure your scenario</p>
                <p className="text-sm mt-2">
                  Issuer: {issuerJurisdiction} | Targets: {targetJurisdictions.join(', ') || 'None'}
                </p>
              </div>
            ) : (
              <>
                {/* Pathway Tab */}
                {activeTab === 'pathway' && (
                  <div className="space-y-4">
                    <div className="flex items-center justify-between">
                      <h3 className="text-lg font-semibold text-white">Compliance Pathway</h3>
                      <StatusBadge
                        status={result.status === 'actionable' ? 'success' : result.status === 'blocked' ? 'error' : 'warning'}
                        label={result.status}
                      />
                    </div>

                    <div className="relative">
                      <div className="absolute left-4 top-0 bottom-0 w-0.5 bg-slate-700" />
                      <div className="space-y-4">
                        {result.pathway.map((step, idx) => (
                          <div key={idx} className="relative pl-10">
                            <div className="absolute left-2 w-5 h-5 rounded-full flex items-center justify-center text-xs font-bold bg-primary-500 text-white">
                              {step.step}
                            </div>
                            <div className="p-4 bg-slate-700 rounded-lg">
                              <div className="flex items-center justify-between mb-2">
                                <span className="font-medium text-white">{step.jurisdiction}</span>
                                {step.timeline_days && (
                                  <span className="text-sm text-slate-400">{step.timeline_days} days</span>
                                )}
                              </div>
                              <p className="text-slate-300">{step.action}</p>
                              {step.dependencies.length > 0 && (
                                <p className="text-xs text-slate-500 mt-2">
                                  Depends on: {step.dependencies.join(', ')}
                                </p>
                              )}
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>
                )}

                {/* Obligations Tab */}
                {activeTab === 'obligations' && (
                  <div className="space-y-4">
                    <h3 className="text-lg font-semibold text-white">Cumulative Obligations</h3>
                    {result.cumulative_obligations.length === 0 ? (
                      <p className="text-slate-400">No obligations identified</p>
                    ) : (
                      <div className="space-y-3">
                        {result.cumulative_obligations.map((ob, idx) => (
                          <div key={idx} className="p-4 bg-slate-700 rounded-lg">
                            <div className="flex items-center gap-2 mb-2">
                              <StatusBadge status="info" label={ob.jurisdiction} size="sm" />
                              <span className="text-xs text-slate-500">{ob.category}</span>
                            </div>
                            <p className="text-slate-300">{ob.description}</p>
                          </div>
                        ))}
                      </div>
                    )}

                    {/* Jurisdiction-specific results */}
                    <h4 className="text-md font-semibold text-white mt-6">By Jurisdiction</h4>
                    {result.jurisdiction_results.map((jr, idx) => (
                      <div key={idx} className="p-4 bg-slate-700 rounded-lg">
                        <div className="flex items-center justify-between mb-3">
                          <span className="font-medium text-white">{jr.jurisdiction}</span>
                          <div className="flex items-center gap-2">
                            <span className="text-xs text-slate-500">{jr.role}</span>
                            <StatusBadge
                              status={jr.status === 'blocked' ? 'error' : 'success'}
                              label={jr.status}
                              size="sm"
                            />
                          </div>
                        </div>
                        <p className="text-sm text-slate-400">{jr.rules_evaluated} rules evaluated</p>
                        {jr.warnings.length > 0 && (
                          <div className="mt-2">
                            {jr.warnings.map((w, i) => (
                              <p key={i} className="text-sm text-amber-400">⚠ {w}</p>
                            ))}
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                )}

                {/* Conflicts Tab */}
                {activeTab === 'conflicts' && (
                  <div className="space-y-4">
                    <h3 className="text-lg font-semibold text-white">Cross-Border Conflicts</h3>
                    {result.conflicts.length === 0 ? (
                      <div className="text-center py-12 text-slate-400">
                        <p className="text-green-400">No conflicts detected</p>
                        <p className="text-sm mt-2">Selected jurisdictions are compatible</p>
                      </div>
                    ) : (
                      <div className="space-y-3">
                        {result.conflicts.map((conflict, idx) => (
                          <div key={idx} className="p-4 bg-slate-700 rounded-lg">
                            <div className="flex items-center gap-2 mb-2">
                              <StatusBadge
                                status={conflict.severity === 'blocking' ? 'error' : 'warning'}
                                label={conflict.type}
                                size="sm"
                              />
                              <span className="text-white font-medium">
                                {conflict.jurisdictions.join(' ↔ ')}
                              </span>
                            </div>
                            <p className="text-slate-300">{conflict.description}</p>
                            {conflict.resolution_hint && (
                              <p className="text-sm text-primary-400 mt-2">
                                <strong>Resolution:</strong> {conflict.resolution_hint}
                              </p>
                            )}
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )}

                {/* Risk Tab */}
                {activeTab === 'risk' && (
                  <div className="space-y-4">
                    <h3 className="text-lg font-semibold text-white">Market Risk Analysis</h3>

                    {/* Token Compliance */}
                    {result.token_compliance && (
                      <div className="p-4 bg-slate-700 rounded-lg">
                        <h4 className="font-medium text-white mb-3">Token Classification</h4>
                        <div className="grid grid-cols-2 gap-4 text-sm">
                          <div>
                            <span className="text-slate-400">Classification:</span>
                            <span className="ml-2 text-white">{result.token_compliance.classification}</span>
                          </div>
                          <div>
                            <span className="text-slate-400">SEC Registration:</span>
                            <StatusBadge
                              status={result.token_compliance.requires_sec_registration ? 'warning' : 'success'}
                              label={result.token_compliance.requires_sec_registration ? 'Required' : 'Not Required'}
                              size="sm"
                            />
                          </div>
                        </div>
                        {result.token_compliance.howey_analysis && (
                          <div className="mt-3 pt-3 border-t border-slate-600">
                            <p className="text-sm text-slate-400 mb-2">Howey Test Analysis:</p>
                            <StatusBadge
                              status={result.token_compliance.howey_analysis.is_security ? 'error' : 'success'}
                              label={result.token_compliance.howey_analysis.is_security ? 'Likely Security' : 'Not a Security'}
                            />
                          </div>
                        )}
                      </div>
                    )}

                    {/* Protocol Risk */}
                    {result.protocol_risk && (
                      <div className="p-4 bg-slate-700 rounded-lg">
                        <h4 className="font-medium text-white mb-3">Protocol Risk Assessment</h4>
                        <div className="grid grid-cols-3 gap-4 text-sm">
                          <div>
                            <span className="text-slate-400">Risk Tier:</span>
                            <StatusBadge
                              status={result.protocol_risk.risk_tier === 'low' ? 'success' : result.protocol_risk.risk_tier === 'high' ? 'error' : 'warning'}
                              label={result.protocol_risk.risk_tier}
                              size="sm"
                            />
                          </div>
                          <div>
                            <span className="text-slate-400">Score:</span>
                            <span className="ml-2 text-white">{(result.protocol_risk.overall_score * 100).toFixed(0)}%</span>
                          </div>
                        </div>
                        {result.protocol_risk.risk_factors.length > 0 && (
                          <div className="mt-3 pt-3 border-t border-slate-600">
                            <p className="text-sm text-amber-400">Risk Factors: {result.protocol_risk.risk_factors.join(', ')}</p>
                          </div>
                        )}
                      </div>
                    )}

                    {/* DeFi Risk */}
                    {result.defi_risk && (
                      <div className="p-4 bg-slate-700 rounded-lg">
                        <h4 className="font-medium text-white mb-3">DeFi Protocol Risk</h4>
                        <div className="grid grid-cols-2 gap-4 text-sm">
                          <div>
                            <span className="text-slate-400">Overall Grade:</span>
                            <span className="ml-2 text-white font-bold">{result.defi_risk.overall_grade}</span>
                          </div>
                          <div>
                            <span className="text-slate-400">Score:</span>
                            <span className="ml-2 text-white">{(result.defi_risk.overall_score * 100).toFixed(0)}%</span>
                          </div>
                        </div>
                        {result.defi_risk.critical_risks.length > 0 && (
                          <div className="mt-3 pt-3 border-t border-slate-600">
                            <p className="text-sm text-red-400">Critical: {result.defi_risk.critical_risks.join(', ')}</p>
                          </div>
                        )}
                      </div>
                    )}

                    {!result.token_compliance && !result.protocol_risk && !result.defi_risk && (
                      <p className="text-slate-400">No market risk analysis available for this scenario.</p>
                    )}
                  </div>
                )}
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
