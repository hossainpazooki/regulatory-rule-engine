/**
 * Tests that each of the four provenance classes renders a VISUALLY DISTINCT
 * badge (distinct color token + accessible label), and that the test-key marker
 * is rendered loudly when requested.
 */
import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ProvenanceClassBadge } from './ProvenanceClassBadge'
import { CLASS_ORDER, CLASS_STYLE, type ProvenanceClass } from './provenance'

describe('ProvenanceClassBadge', () => {
  it.each(CLASS_ORDER)('renders the %s class with its stable color token', (klass) => {
    const { container } = render(<ProvenanceClassBadge klass={klass} />)
    const badge = container.querySelector(`[data-provenance-class="${klass}"]`)
    expect(badge).not.toBeNull()
    // every color-class token from the map is applied
    for (const token of CLASS_STYLE[klass].colorClass.split(' ')) {
      expect(badge).toHaveClass(token)
    }
    expect(badge).toHaveAttribute('aria-label', CLASS_STYLE[klass].aria)
  })

  it('renders the four classes with four DISTINCT color tokens', () => {
    const seen = CLASS_ORDER.map((klass) => {
      const { container } = render(<ProvenanceClassBadge klass={klass} />)
      const badge = container.querySelector(
        `[data-provenance-class="${klass}"]`,
      ) as HTMLElement
      return badge.className
    })
    expect(new Set(seen).size).toBe(4)
  })

  it('renders a loud TEST KEY marker only when isTestKey is true', () => {
    const { rerender } = render(
      <ProvenanceClassBadge klass={'compiler-validity' as ProvenanceClass} />,
    )
    expect(screen.queryByTestId('test-key-marker')).toBeNull()

    rerender(<ProvenanceClassBadge klass="compiler-validity" isTestKey />)
    const marker = screen.getByTestId('test-key-marker')
    expect(marker).toHaveTextContent('TEST KEY - not production')
    expect(marker).toHaveClass('text-red-300')
  })
})
