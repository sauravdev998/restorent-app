import js from '@eslint/js'
import query from '@tanstack/eslint-plugin-query'
import prettier from 'eslint-config-prettier'
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
  { ignores: ['dist', 'src/shared/api/schema.d.ts'] },

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
    },
  },

  {
    files: ['**/*.test.{ts,tsx}', 'src/test/**'],
    rules: {
      '@typescript-eslint/no-unsafe-assignment': 'off',
      '@typescript-eslint/no-unsafe-member-access': 'off',
      '@typescript-eslint/unbound-method': 'off',
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
