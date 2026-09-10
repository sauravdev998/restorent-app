import { expect, test, type Browser, type Page } from '@playwright/test'

/**
 * The one claim this whole product rests on, checked the only way it can be.
 *
 * Two browser contexts, open at the same time, signed in as two different
 * people. The waiter sends a dish; the chef's screen shows the ticket without
 * anybody touching it. The chef marks the dishes done; the waiter's screen
 * raises the alert without anybody touching it either. Then the food is carried
 * out, the bill closes with a number, and the table goes back to free.
 *
 * Nothing cheaper proves this. The API tests prove the rows are right, and the
 * unit tests prove the fan out map invalidates the correct keys, and both would
 * pass on a build where the event stream never reaches the browser at all. This
 * is the test that fails on that build.
 *
 * **No reload anywhere in it, and that is the assertion.** Every wait below is
 * on content appearing in a page that is already open, so a build where the
 * stream is dead cannot pass by refetching on navigation. The one place the
 * waiter's page navigates is between the floor and the table, which is a real
 * user action rather than a workaround.
 *
 * It needs a seeded database and a running API. `pnpm db:seed` makes both
 * accounts and the four tables; the accounts are in `.env.example`.
 */

const WAITER = { email: 'waiter@example.test', password: 'development-only-password' }
const CHEF = { email: 'chef@example.test', password: 'development-only-password' }

/** Signs a person in on their own context, so the two hold separate cookies. */
async function signIn(browser: Browser, who: { email: string; password: string }): Promise<Page> {
  const context = await browser.newContext()
  const page = await context.newPage()

  await page.goto('/sign-in')
  await page.getByLabel(/email/i).fill(who.email)
  await page.getByLabel(/password/i).fill(who.password)
  await page.getByRole('button', { name: /sign in/i }).click()

  return page
}

test('a dish sent by a waiter reaches the kitchen live, and back again', async ({ browser }) => {
  const waiter = await signIn(browser, WAITER)
  const chef = await signIn(browser, CHEF)

  // Each lands on their own surface, which is the role routing working.
  await expect(waiter.getByRole('heading', { name: /floor/i })).toBeVisible()
  await expect(chef.getByRole('heading', { name: /the pass/i })).toBeVisible()

  // The chef's screen is open and stays open from here on. Nothing below
  // reloads it, so anything that appears on it arrived over the stream.

  // A free table. Whichever one is free, so a run against a database that
  // already has a table open still has somewhere to sit.
  const openButton = waiter.getByRole('button', { name: /^open table$/i }).first()
  await expect(openButton).toBeVisible()
  await openButton.click()

  // Into the table's own screen, which is a real navigation a waiter makes.
  const heading = waiter.getByRole('heading', { level: 1, name: /^table /i })
  await expect(heading).toBeVisible()

  // Which table this run got. Everything on the chef's side is found by this
  // rather than by "the first ticket", because the pass is shared: a leftover
  // ticket from an earlier run would otherwise be the one this test marked
  // done, and the failure would read as the live path being broken.
  const label = ((await heading.textContent()) ?? '').replace(/^table\s+/i, '').trim()
  expect(label).not.toBe('')

  // Two things about this pattern, both learned the hard way, because a locator
  // that matches nothing fails identically to a live path that is broken.
  //
  // Not anchored with `^`: `hasText` matches an element's whole `textContent`,
  // which carries the whitespace JSX leaves between nodes.
  //
  // And `(?!\\d)` rather than `\\b`: the card's text content runs its spans
  // together as `Table 1Round 1`, so there is no word boundary after the label
  // at all. The lookahead says what was actually meant, which is that table 1
  // must not match table 10.
  const ticket = chef
    .getByRole('listitem')
    .filter({ hasText: new RegExp(`Table ${label}(?!\\d)`) })
    .first()

  await expect(ticket).toBeHidden()

  // Two dishes, so "one dish done does not make the ticket ready" is exercised
  // rather than assumed.
  const addButtons = waiter.getByRole('button', { name: /^add one /i })
  await addButtons.nth(0).click()
  await addButtons.nth(1).click()

  await waiter.getByRole('button', { name: /^send /i }).click()

  // The claim. No reload, no click, no navigation on the chef's page.
  await expect(ticket).toBeVisible()

  // Two dishes on it, each with its own button, because a chef marks one dish
  // at a time as it comes off the pass.
  const done = ticket.getByRole('button', { name: /^done$/i })
  await expect(done).toHaveCount(2)

  await done.first().click()

  // Wait for that request to land before tapping the next dish. The button's
  // own label changes to "Marking" the instant it is tapped, so counting the
  // "Done" buttons alone would let the second tap overlap the first, and this
  // scenario would then be quietly testing two concurrent writes instead of
  // the thread. That overlap is real and worth covering, and it is covered
  // deterministically by `two_dishes_on_one_ticket_marked_at_once_still_leave_it_ready`
  // in the API's concurrency suite rather than by timing here.
  await expect(ticket.getByRole('button', { name: /^marking$/i })).toHaveCount(0)

  // One dish done leaves the ticket queued: the waiter must not be sent to
  // collect half an order.
  await expect(done).toHaveCount(1)

  await done.first().click()

  // The last dish makes the whole ticket ready by itself, and the waiter's
  // screen says so without being touched.
  //
  // Two assertions, because the alert is meant to arrive on two channels and a
  // waiter who is not looking at the phone depends on the second. A single
  // loose text match would find either one and pass on a build that had lost
  // the other.
  const alerted = `Table ${label}, round 1 is ready`

  // The badge.
  await expect(waiter.getByText(alerted, { exact: true })).toBeVisible()

  // And the announcement, through the one live region the shell mounts.
  await expect(waiter.locator('[aria-live="assertive"]')).toContainText(alerted)

  // Carried out. The ticket leaves the pass, again with no reload.
  await waiter.getByRole('button', { name: /^mark served$/i }).click()
  await expect(ticket).toBeHidden()

  // And the meal ends with a number and a total.
  await waiter.getByRole('button', { name: /^close the bill$/i }).click()
  await expect(waiter.getByRole('heading', { name: /^bill \d+$/i })).toBeVisible()
  await expect(waiter.getByText(/^total$/i)).toBeVisible()

  await waiter.close()
  await chef.close()
})
