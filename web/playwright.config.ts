import { defineConfig, devices } from '@playwright/test'

/**
 * The browser tests.
 *
 * There is one scenario and there is meant to be one. Browser tests are the
 * slowest and flakiest thing a pipeline runs, and this one starts a database, an
 * API, and a web build to check a single claim. That claim happens to be the
 * one the whole product rests on: a dish sent on a waiter's phone appears on a
 * kitchen screen across the room without anybody refreshing anything. It cannot
 * be checked any other way, because both halves are real browsers holding real
 * event streams, and everything cheaper than this proves something adjacent to
 * it instead.
 *
 * The web server is the production build served by `vite preview`, not the dev
 * server. The dev server injects a hot reload client that holds its own
 * connection and reconnects on its own schedule, which is noise in a test whose
 * entire subject is a connection staying open. The `/api` proxy is configured
 * the same way in both, so the thing under test is unchanged.
 *
 * `workers: 1` because the two contexts share one seeded restaurant with four
 * tables. Two scenarios running at once would race for a table and the failure
 * would look like the live path being broken rather than the test being greedy.
 */
/**
 * How long any one step may wait, in milliseconds.
 *
 * See the note on `timeout` below for why this is a knob rather than a
 * constant, and why the default is the tight one.
 */
const stepTimeout = Number(process.env['E2E_TIMEOUT_MS'] ?? 30_000)

export default defineConfig({
  testDir: './e2e',
  // How long any one step may wait, and the one number to turn up when this is
  // run against a database that is not next door.
  //
  // Deliberately not a measurement of AC-5's "about two seconds". That claim is
  // about an API and its database sharing a network, which is what continuous
  // integration gives it: a Postgres container in the same job, where the whole
  // scenario runs in seconds and this bound is tight enough to mean something.
  //
  // A developer's machine often talks to a hosted database instead, and every
  // repository operation is a handful of sequential statements, so one send is
  // roughly twenty internet round trips. Measured on such a link: twenty two
  // seconds for one send, and the whole thread from tap to alert anywhere
  // between thirty seconds and two minutes. Set `E2E_TIMEOUT_MS` there rather
  // than raising the default and blunting the check for everybody.
  //
  // What this bound catches either way is "the ticket never arrives", which is
  // the failure the scenario exists for. Timing the claim itself belongs to a
  // measurement against a real deployment, not to a browser test.
  timeout: stepTimeout * 6,
  expect: { timeout: stepTimeout },
  workers: 1,
  // A browser test that passes on the second attempt has told you nothing about
  // the first. Retries hide exactly the intermittency this scenario exists to
  // catch, so there are none, here or in continuous integration.
  retries: 0,
  reporter: process.env['CI'] ? [['github'], ['list']] : [['list']],
  use: {
    baseURL: process.env['E2E_BASE_URL'] ?? 'http://127.0.0.1:4173',
    trace: 'retain-on-failure',
    video: 'off',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  // Skipped when a base URL is supplied, so the same scenario can be pointed at
  // an already running stack while somebody is debugging it.
  ...(process.env['E2E_BASE_URL']
    ? {}
    : {
        webServer: {
          // `--host 127.0.0.1` is not cosmetic. Vite's preview server binds
          // to `localhost`, which on a machine with IPv6 resolves to `::1`
          // first, and Playwright polls the literal `127.0.0.1` below: the
          // server comes up fine and the wait times out anyway.
          command: 'pnpm build && pnpm preview --port 4173 --strictPort --host 127.0.0.1',
          url: 'http://127.0.0.1:4173',
          reuseExistingServer: !process.env['CI'],
          timeout: 180_000,
        },
      }),
})
