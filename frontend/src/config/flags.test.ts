/**
 * Unit tests for the Gate-5 flag parser. We test `flagOn` directly — the single
 * source of truth for parsing — rather than the module-level constants, which
 * would require brittle `import.meta.env` mutation. The constants are just
 * `flagOn(import.meta.env.VITE_USE_*)`, so a green truth table here proves the
 * default-OFF contract for all three flags.
 */

import { describe, it, expect } from 'vitest'
import { flagOn } from './flags'

describe('flagOn', () => {
  it('is true ONLY for the exact string "true" (case-insensitive, trimmed)', () => {
    expect(flagOn('true')).toBe(true)
    expect(flagOn('TRUE')).toBe(true)
    expect(flagOn('True')).toBe(true)
    expect(flagOn(' true ')).toBe(true)
    expect(flagOn('  TRUE  ')).toBe(true)
  })

  it('defaults OFF for undefined / empty / falsey strings', () => {
    expect(flagOn(undefined)).toBe(false)
    expect(flagOn('')).toBe(false)
    expect(flagOn('false')).toBe(false)
    expect(flagOn('FALSE')).toBe(false)
    expect(flagOn('0')).toBe(false)
    expect(flagOn('1')).toBe(false)
  })

  it('defaults OFF for arbitrary non-"true" strings', () => {
    expect(flagOn('yes')).toBe(false)
    expect(flagOn('on')).toBe(false)
    expect(flagOn('truthy')).toBe(false)
    expect(flagOn('truex')).toBe(false)
    expect(flagOn('t r u e')).toBe(false)
  })

  it('honors a real boolean directly', () => {
    expect(flagOn(true)).toBe(true)
    expect(flagOn(false)).toBe(false)
  })
})
