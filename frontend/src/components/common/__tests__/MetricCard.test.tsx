import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MetricCard } from '../MetricCard'

describe('MetricCard', () => {
  it('renders title and value', () => {
    render(<MetricCard title="Total Users" value={1234} />)
    expect(screen.getByText('Total Users')).toBeInTheDocument()
    expect(screen.getByText('1234')).toBeInTheDocument()
  })

  it('renders string value', () => {
    render(<MetricCard title="Status" value="Active" />)
    expect(screen.getByText('Active')).toBeInTheDocument()
  })

  it('renders subtitle when provided', () => {
    render(<MetricCard title="Revenue" value="$10,000" subtitle="This month" />)
    expect(screen.getByText('This month')).toBeInTheDocument()
  })

  it('does not render subtitle when not provided', () => {
    render(<MetricCard title="Revenue" value="$10,000" />)
    expect(screen.queryByText('This month')).not.toBeInTheDocument()
  })

  it('renders icon when provided', () => {
    render(
      <MetricCard
        title="Users"
        value={42}
        icon={<span data-testid="test-icon">Icon</span>}
      />
    )
    expect(screen.getByTestId('test-icon')).toBeInTheDocument()
  })

  it('renders upward trend with green color', () => {
    render(<MetricCard title="Growth" value="15%" trend="up" trendValue="+5%" />)
    const trendElement = screen.getByText(/\+5%/)
    expect(trendElement).toHaveClass('text-green-400')
    expect(trendElement.textContent).toContain('↑')
  })

  it('renders downward trend with red color', () => {
    render(<MetricCard title="Churn" value="2%" trend="down" trendValue="-1%" />)
    const trendElement = screen.getByText(/-1%/)
    expect(trendElement).toHaveClass('text-red-400')
    expect(trendElement.textContent).toContain('↓')
  })

  it('renders neutral trend with slate color', () => {
    render(<MetricCard title="Stable" value="100" trend="neutral" trendValue="±0%" />)
    const trendElement = screen.getByText(/±0%/)
    expect(trendElement).toHaveClass('text-slate-400')
    expect(trendElement.textContent).toContain('→')
  })

  it('does not render trend when only trend is provided without trendValue', () => {
    render(<MetricCard title="Test" value={100} trend="up" />)
    expect(screen.queryByText('↑')).not.toBeInTheDocument()
  })

  it('applies custom className', () => {
    const { container } = render(
      <MetricCard title="Test" value={100} className="custom-metric" />
    )
    const card = container.firstChild
    expect(card).toHaveClass('custom-metric')
  })
})
