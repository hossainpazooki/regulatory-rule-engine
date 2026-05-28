import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { StatusBadge } from '../StatusBadge'

describe('StatusBadge', () => {
  it('renders the label text', () => {
    render(<StatusBadge status="success" label="Active" />)
    expect(screen.getByText('Active')).toBeInTheDocument()
  })

  it('applies success styles', () => {
    render(<StatusBadge status="success" label="Success" />)
    const badge = screen.getByText('Success')
    expect(badge).toHaveClass('text-green-400')
  })

  it('applies warning styles', () => {
    render(<StatusBadge status="warning" label="Warning" />)
    const badge = screen.getByText('Warning')
    expect(badge).toHaveClass('text-yellow-400')
  })

  it('applies error styles', () => {
    render(<StatusBadge status="error" label="Error" />)
    const badge = screen.getByText('Error')
    expect(badge).toHaveClass('text-red-400')
  })

  it('applies info styles', () => {
    render(<StatusBadge status="info" label="Info" />)
    const badge = screen.getByText('Info')
    expect(badge).toHaveClass('text-blue-400')
  })

  it('applies neutral styles', () => {
    render(<StatusBadge status="neutral" label="Neutral" />)
    const badge = screen.getByText('Neutral')
    expect(badge).toHaveClass('text-slate-400')
  })

  it('renders with default size (md)', () => {
    render(<StatusBadge status="success" label="Default" />)
    const badge = screen.getByText('Default')
    expect(badge).toHaveClass('px-3', 'py-1', 'text-sm')
  })

  it('renders with small size', () => {
    render(<StatusBadge status="success" label="Small" size="sm" />)
    const badge = screen.getByText('Small')
    expect(badge).toHaveClass('px-2', 'py-0.5', 'text-xs')
  })
})
