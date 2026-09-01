import { render } from '@testing-library/react'
import * as axe from 'axe-core'
import type { ReactElement } from 'react'
import { expect } from 'vitest'

/** Both appearances, because a light theme nobody tests is a light theme that rots. */
export const APPEARANCES = ['dark', 'light'] as const

/** All three rooms, because a control that is big enough at a desk may not be in a kitchen. */
export const SURFACES = ['admin', 'waiter', 'kitchen'] as const

/**
 * axe's own `color-contrast` rule is off, everywhere, deliberately.
 *
 * jsdom has no layout and paints nothing, so the rule cannot see what is
 * actually behind an element and reports whatever it can guess. Colour is owned
 * end to end by `scripts/check-contrast.ts`, which reads the real stylesheet.
 * Two gates both claiming colour, one of them unreliable, is worse than one
 * gate that is trusted.
 */
const SHARED_RULES = {
  'color-contrast': { enabled: false },
} as const

/**
 * Rules about the shape of a whole document, switched off when the thing under
 * test is one component rather than a page. A `Button` on its own is not
 * missing a `<main>`; it is a button.
 */
const COMPONENT_ONLY_RULES = {
  region: { enabled: false },
  'page-has-heading-one': { enabled: false },
  'landmark-one-main': { enabled: false },
  'html-has-lang': { enabled: false },
} as const

export interface AccessibleOptions {
  /**
   * Treat the subject as a whole page, so the landmark and heading rules run
   * too. Use it for anything that renders a `<main>`.
   */
  page?: boolean
}

/**
 * Renders `ui` in every appearance and every density and fails if axe finds
 * anything.
 *
 * Six renders per component, and that is the point: a component is only proved
 * where it was actually run, and this platform ships the same component into
 * three very different rooms and two very different appearances.
 */
export async function expectAccessible(
  ui: ReactElement,
  options: AccessibleOptions = {},
): Promise<void> {
  const config: axe.RunOptions = {
    rules: { ...SHARED_RULES, ...(options.page === true ? {} : COMPONENT_ONLY_RULES) },
  }

  for (const theme of APPEARANCES) {
    for (const surface of SURFACES) {
      const { container, unmount } = render(
        <div data-theme={theme} data-surface={surface}>
          {ui}
        </div>,
      )

      const results = await axe.run(container, config)

      if (results.violations.length > 0) {
        const detail = results.violations
          .map((violation) => `  ${violation.id}: ${violation.help}`)
          .join('\n')
        unmount()
        expect.fail(
          `axe found violations in the ${theme} appearance at ${surface} density:\n${detail}`,
        )
      }

      unmount()
    }
  }
}
