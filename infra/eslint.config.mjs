import js from '@eslint/js'
import prettier from 'eslint-config-prettier'
import tseslint from 'typescript-eslint'

export default tseslint.config(
  { ignores: ['cdk.out', 'dist'] },

  js.configs.recommended,

  {
    // Type aware rules need a tsconfig that covers the file, and this project's
    // tsconfig includes only `bin/` and `lib/`. Applying them repo wide would
    // break on this config file, which nothing includes.
    files: ['{bin,lib}/**/*.ts'],
    extends: [tseslint.configs.recommendedTypeChecked],
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      // A leading underscore is how you say "this exists to satisfy a
      // signature" out loud. Same rule as `web/`, so the two read alike.
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
      '@typescript-eslint/no-non-null-assertion': 'error',
    },
  },

  {
    // CDK constructs are built for their side effect on the tree: `new
    // Distribution(this, ...)` is the whole point, and its return value is
    // meant to be dropped. Everywhere else this rule is worth keeping.
    files: ['lib/**/*.ts'],
    rules: {
      'no-new': 'off',
    },
  },

  // Must stay last: it switches off every rule Prettier already handles, so the
  // two can never disagree about formatting.
  prettier,
)
