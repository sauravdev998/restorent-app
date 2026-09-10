import js from '@eslint/js'
import query from '@tanstack/eslint-plugin-query'
import prettier from 'eslint-config-prettier'
import i18next from 'eslint-plugin-i18next'
import jsxA11y from 'eslint-plugin-jsx-a11y'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import globals from 'globals'
import tseslint from 'typescript-eslint'

/**
 * Tailwind utilities that write a physical direction.
 *
 * Every one of them has a logical twin: `ml-*` becomes `ms-*`, `pr-*` becomes
 * `pe-*`, `text-left` becomes `text-start`, `border-l` becomes `border-s`.
 * Writing the physical one works fine until the day a right to left language is
 * added, at which point the layout mirrors everywhere except the places someone
 * hardcoded, and those places are found one screenshot at a time.
 *
 * The trailing group matters: it is what keeps `rounded-lg` and `border-line`
 * legal, since both begin with a banned prefix and neither is physical.
 */
const PHYSICAL_DIRECTION_UTILITIES = String.raw`(^|[\s:])(m[lr]|p[lr]|scroll-m[lr]|scroll-p[lr]|border-[lr]|rounded-[lr]|rounded-[tb][lr]|inset-[lr]|left|right|text-(left|right)|float-(left|right)|clear-(left|right))(-[^\s]*)?(\s|$)`

const PHYSICAL_DIRECTION_MESSAGE =
  'Use the logical property, not the physical one: ms-/me- instead of ml-/mr-, ps-/pe- instead of pl-/pr-, start-/end- instead of left-/right-, text-start/text-end instead of text-left/text-right, border-s/border-e instead of border-l/border-r. Setting dir="rtl" has to mirror the layout without anyone editing a component.'

/**
 * Attributes that carry words a person reads.
 *
 * The visible ones (`alt`, `title`, `placeholder`) plus the ones only a screen
 * reader ever reaches, which is exactly where an untranslated string survives
 * longest because nobody looking at the screen can see it. `label`, `hint`, and
 * `description` are this project's own component props for the same thing:
 * `Field`, `Icon`, `Alert`, and the toast store all take their words that way,
 * so leaving them out would exempt most of the text in the design system.
 */
const TEXT_CARRYING_ATTRIBUTES = [
  'alt',
  'title',
  'placeholder',
  'label',
  'hint',
  'description',
  'aria-label',
  'aria-placeholder',
  'aria-roledescription',
  'aria-valuetext',
]

const NO_LITERAL_STRING_MESSAGE =
  'No user facing string is written into a component. Move it to src/locales/<lang>/<namespace>.json and read it through t(). If this string is not user facing (a test id, a code, a class name), say so with an eslint-disable-next-line and a reason beside it.'

const noPhysicalDirection = [
  {
    selector: `JSXAttribute[name.name='className'] Literal[value=/${PHYSICAL_DIRECTION_UTILITIES}/]`,
    message: PHYSICAL_DIRECTION_MESSAGE,
  },
  {
    selector: `CallExpression[callee.name=/^(cn|cva)$/] Literal[value=/${PHYSICAL_DIRECTION_UTILITIES}/]`,
    message: PHYSICAL_DIRECTION_MESSAGE,
  },
]

export default tseslint.config(
  // The generated client is generated. Linting it would only ever produce
  // complaints nobody can act on without editing a file they must not edit.
  // Playwright writes traces, screenshots, and copies of the source under
  // these when a browser test fails. They are build output, not sources.
  { ignores: ['dist', 'test-results', 'playwright-report', 'src/shared/api/schema.d.ts'] },

  js.configs.recommended,

  {
    // Type aware rules need a tsconfig covering the file, so they are scoped to
    // the TypeScript sources. Applying them repo wide would break on this very
    // config file, which no tsconfig includes.
    files: ['**/*.{ts,tsx}'],
    extends: [
      tseslint.configs.recommendedTypeChecked,
      reactHooks.configs.flat['recommended-latest'],
      reactRefresh.configs.vite,
      // Strict, not recommended. The difference is the rules that catch a
      // control nobody can reach: a click handler on a div, an interactive
      // element with no keyboard equivalent, a label pointing at nothing.
      jsxA11y.flatConfigs.strict,
      ...query.configs['flat/recommended'],
    ],
    languageOptions: {
      ecmaVersion: 2023,
      globals: globals.browser,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      // A leading underscore is how you say "this exists to satisfy a
      // signature" out loud.
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
      '@typescript-eslint/no-non-null-assertion': 'error',
      'no-restricted-syntax': ['error', ...noPhysicalDirection],
      // A named region is allowed to be focusable, on top of the rule's own
      // `tabpanel`. This is the sanctioned fix for a box that scrolls: axe
      // fails a scrollable container nobody can focus, because a keyboard or
      // switch user can never reach the far side of it, and the remedy is
      // exactly `role="region"` plus a name plus `tabIndex={0}`. Without this
      // the two rules contradict each other and one of them has to be muted at
      // every call site instead of decided once, here.
      'jsx-a11y/no-noninteractive-tabindex': [
        'error',
        { tags: [], roles: ['tabpanel', 'region'], allowExpressionValues: true },
      ],
    },
  },

  {
    // The literal string gate, and the whole reason spec 0005 exists: a screen
    // with English baked into it stops the pull request rather than being found
    // by a translator six months later.
    //
    // Scoped to the interface sources. Tests assert on the English words a
    // screen renders, so they are exempt below; `scripts/` is Node tooling
    // nobody reads; and the generated client is already ignored entirely.
    files: ['src/**/*.tsx'],
    // The design gallery is the one exception, and it is a narrow one. It is a
    // development only route that never reaches a production build, and what
    // is left in it after the real prose moved into the `admin` namespace is
    // sample data rather than copy: a fake bill line showing tabular figures,
    // the three size names beside the buttons they size, a made up table and
    // round on a card. Every one of those is there to show a typeface or a
    // spacing step, and putting them through a translation file would ask a
    // translator to translate "md".
    ignores: ['src/app/design/**'],
    plugins: { i18next },
    rules: {
      'i18next/no-literal-string': [
        'error',
        {
          // JSX text and the attributes named above, and nothing else. `all`
          // would flag every string in the file, including class names and
          // object keys, which turns the gate into noise people learn to
          // ignore.
          mode: 'jsx-only',
          'jsx-attributes': { include: TEXT_CARRYING_ATTRIBUTES },
          message: NO_LITERAL_STRING_MESSAGE,
        },
      ],
    },
  },

  {
    files: ['**/*.test.{ts,tsx}', 'src/test/**'],
    rules: {
      '@typescript-eslint/no-unsafe-assignment': 'off',
      '@typescript-eslint/no-unsafe-member-access': 'off',
      '@typescript-eslint/unbound-method': 'off',
      // A test asserts on the words a screen actually renders. Reading them
      // from the same file the component reads would assert only that the two
      // agree with each other, which is not the same thing at all.
      'i18next/no-literal-string': 'off',
    },
  },

  {
    // Node scripts, not browser code.
    files: ['scripts/**/*.ts'],
    languageOptions: { globals: globals.node },
  },

  // Must stay last: it switches off every rule Prettier already handles, so the
  // two can never disagree about formatting.
  prettier,
)
