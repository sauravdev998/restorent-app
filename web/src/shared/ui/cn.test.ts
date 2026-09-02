import { describe, expect, it } from 'vitest'

import { cn } from './cn'

describe('cn', () => {
  it('keeps a density border width alongside a border colour', () => {
    // The regression: tailwind-merge reads `border-line` as a border colour,
    // so out of the box it dropped the width and left every card, button, pill,
    // and input with no boundary at all. Invisible at a desk, and exactly the
    // thing the kitchen surface and forced colours mode depend on.
    expect(cn('border-line', 'border-border')).toContain('border-line')
    expect(cn('border-line', 'border-border')).toContain('border-border')
    expect(cn('border-b-line', 'border-border')).toContain('border-b-line')
    expect(cn('border-t-line', 'border-border')).toContain('border-t-line')
    expect(cn('stroke-token', 'text-primary')).toContain('stroke-token')
  })

  it('still lets a real conflict resolve to the last one', () => {
    expect(cn('p-4', 'p-6')).toBe('p-6')
    expect(cn('border-line', 'border-2')).toBe('border-2')
    expect(cn('text-status-ready', 'text-status-late')).toBe('text-status-late')
  })
})
