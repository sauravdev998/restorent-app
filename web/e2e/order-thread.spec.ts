import { expect, test, type Browser, type Page } from '@playwright/test'

/**
 * The one claim this whole product rests on, checked the only way it can be.
 *
 * Two browser contexts, open at the same time, signed in as two different
 * people. The waiter sends a round with a note; the chef's screen shows the
 * ticket and the note, whole, without anybody touching it. The waiter walks to
 * the Orders list. The chef marks one dish ready; the waiter's screen raises
 * the alert there, without anybody touching it, and the waiter serves that one
 * dish. A second round goes to the kitchen as a ticket of its own. Then the
 * rest is carried out, the bill closes with a number, and the table goes back
 * to free (spec 0011, AC-20).
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
  const tickets = chef
    .getByRole('listitem')
    .filter({ hasText: new RegExp(`Table ${label}(?!\\d)`) })

  await expect(tickets).toHaveCount(0)

  // Two dishes, so "one dish ready does not make the ticket ready" is exercised
  // rather than assumed. Orderable ones only: the admin can put a dish the
  // kitchen has switched off anywhere in the menu (spec 0008), so the first
  // two buttons on screen are not necessarily two that can be pressed.
  const addButtons = waiter.getByRole('button', { name: /^add one /i, disabled: false })
  const firstDish = ((await addButtons.nth(0).getAttribute('aria-label')) ?? '').replace(
    /^add one /i,
    '',
  )
  const secondDish = ((await addButtons.nth(1).getAttribute('aria-label')) ?? '').replace(
    /^add one /i,
    '',
  )
  expect(firstDish).not.toBe('')
  expect(secondDish).not.toBe('')

  await addButtons.nth(0).click()
  await addButtons.nth(1).click()

  // A note on the first dish, which has to reach the pass exactly as typed
  // (spec 0011, AC-6).
  const note = 'No onions, nut allergy at this table'
  await waiter.getByRole('textbox', { name: `Note for ${firstDish}` }).fill(note)

  await waiter.getByRole('button', { name: /^send /i }).click()

  // The claim. No reload, no click, no navigation on the chef's page.
  // By the round's own label as a whole element, not by a pattern over the
  // card's text: the elapsed time follows it directly, so "Round 1" runs into
  // "1 minute" and no lookahead can tell the two apart.
  const firstTicket = tickets.filter({ has: chef.getByText('Round 1', { exact: true }) })
  await expect(firstTicket).toBeVisible()
  await expect(firstTicket.getByText(note, { exact: true })).toBeVisible()

  // Two dishes on it, each with its own button, because a chef marks one dish
  // at a time as it comes off the pass.
  const done = firstTicket.getByRole('button', { name: /^done$/i })
  await expect(done).toHaveCount(2)

  // The waiter walks to the Orders list, away from the table, which is the
  // point: the alert has to find them wherever they are (spec 0011, AC-9).
  await waiter.getByRole('link', { name: /^orders$/i }).click()
  await expect(waiter.getByRole('heading', { level: 1, name: /^orders$/i })).toBeVisible()

  // One dish comes off the pass. The ticket stays cooking, but that dish is
  // ready, and it is the dish that matters to the waiter now.
  await done.first().click()
  await expect(firstTicket.getByRole('button', { name: /^marking$/i })).toHaveCount(0)
  await expect(done).toHaveCount(1)

  // The alert, on a screen that is not the table's, with no reload. Seen, and
  // said through the one live region the shell mounts, because a waiter who
  // is not looking at the phone depends on the second.
  //
  // Scoped to this run's table: the shared development restaurant can hold
  // other ready food this waiter is responsible for, which the alert rightly
  // shows too.
  const alert = waiter.getByRole('region', { name: /ready to collect$/i })
  const alertedHere = alert
    .getByRole('listitem')
    .filter({ has: waiter.getByRole('link', { name: `Table ${label}`, exact: true }) })
  await expect(alertedHere).toContainText(`1 × ${firstDish}`)
  await expect(waiter.locator('[aria-live="assertive"]')).toContainText(
    `Ready at table ${label}: 1 × ${firstDish}.`,
  )

  // Carried out on its own, from this table's card on the Orders list, while
  // the other dish cooks (spec 0011, AC-12).
  const card = waiter
    .getByRole('main')
    .getByRole('listitem')
    .filter({ has: waiter.getByRole('heading', { level: 2, name: `Table ${label}`, exact: true }) })
  await card.getByRole('button', { name: `Serve ${firstDish}` }).click()
  await expect(alertedHere).toHaveCount(0)

  // Back to the table for a second round, which reaches the kitchen as a
  // ticket of its own (spec 0011, AC-5).
  await card.getByRole('link', { name: `Table ${label}`, exact: true }).click()
  await expect(heading).toBeVisible()
  await waiter.getByRole('button', { name: `Add one ${secondDish}` }).click()
  await waiter.getByRole('button', { name: /^send /i }).click()

  const secondTicket = tickets.filter({ has: chef.getByText('Round 2', { exact: true }) })
  await expect(secondTicket).toBeVisible()
  await expect(firstTicket).toBeVisible()

  // The rest of the meal, so the table is free again for the next run.
  await done.first().click()
  await expect(done).toHaveCount(0)
  await secondTicket.getByRole('button', { name: /^done$/i }).click()

  const serveAll = waiter.getByRole('button', { name: /^serve all ready$/i })
  await expect(serveAll).toHaveCount(2)
  await serveAll.first().click()
  await expect(serveAll).toHaveCount(1)
  await serveAll.first().click()
  await expect(serveAll).toHaveCount(0)
  await expect(tickets).toHaveCount(0)

  // And the meal ends with a number and a total.
  await waiter.getByRole('button', { name: /^close the bill$/i }).click()
  await expect(waiter.getByRole('heading', { name: /^bill \d+$/i })).toBeVisible()
  await expect(waiter.getByText(/^total$/i)).toBeVisible()

  await waiter.close()
  await chef.close()
})
