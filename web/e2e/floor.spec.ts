import { expect, test, type Browser, type Page } from '@playwright/test'

/**
 * The floor's one live claim, checked the only way it can be: an admin and a
 * waiter on two screens at once. Spec 0010 AC-20.
 *
 * The admin adds a table and the waiter's floor shows it without being
 * touched; the admin removes it and it leaves the waiter's floor the same way.
 *
 * **No reload anywhere, and that is the assertion.** A build where the event
 * stream never reaches the browser must not be able to pass by refetching on
 * navigation. The waiter's page is opened once and never navigated again.
 *
 * **It never opens a table.** The seeded restaurant has four, and the order
 * thread's scenario needs one of them free.
 *
 * It needs a seeded database and a running API. The table's label is unique to
 * the run, within the 12 characters a label may hold, so a database that has
 * seen this scenario before still gives it a clean table to watch. Each run
 * leaves one archived table behind, which only the admin's Archived list shows.
 */

const ADMIN = { email: 'admin@example.test', password: 'development-only-password' }
const WAITER = { email: 'waiter@example.test', password: 'development-only-password' }

/** Signs a person in on their own context, so each holds its own cookie. */
async function signIn(browser: Browser, who: { email: string; password: string }): Promise<Page> {
  const context = await browser.newContext()
  const page = await context.newPage()

  await page.goto('/sign-in')
  await page.getByLabel(/email/i).fill(who.email)
  await page.getByLabel(/password/i).fill(who.password)
  await page.getByRole('button', { name: /sign in/i }).click()

  return page
}

test('a table the admin adds and then removes comes and goes on the waiter floor live', async ({
  browser,
}) => {
  // "E2E" and the last nine digits of the clock: 12 characters, unique per run.
  const label = `E2E${String(Date.now()).slice(-9)}`

  const admin = await signIn(browser, ADMIN)
  const waiter = await signIn(browser, WAITER)

  // The waiter's floor opens first and stays open from here on.
  await expect(waiter.getByRole('heading', { level: 1, name: /floor/i })).toBeVisible()
  const onWaiterFloor = waiter.getByText(label, { exact: true })
  await expect(onWaiterFloor).toBeHidden()

  // The admin walks to the floor screen the way an admin would.
  await admin.getByRole('link', { name: /^tables$/i }).click()
  await expect(admin.getByRole('heading', { level: 1, name: /^tables$/i })).toBeVisible()

  await admin.getByRole('button', { name: /^add tables$/i }).click()
  const form = admin.getByRole('dialog')
  await form.getByLabel(/^label/i).fill(label)
  await form.getByLabel(/^seats/i).fill('6')
  await form.getByRole('button', { name: /^add table$/i }).click()
  await expect(form).toBeHidden()

  // On the admin's own screen at once, from the refetch its save triggered.
  // Inside the page itself, so the "added" toast is not mistaken for the row.
  const row = admin.getByRole('main').getByRole('listitem').filter({ hasText: label })
  await expect(row).toBeVisible()

  // The first claim. No reload, no click, no navigation on the waiter's page.
  await expect(onWaiterFloor).toBeVisible()
  const card = waiter.getByRole('main').getByRole('listitem').filter({ hasText: label })
  await expect(card.getByText('Seats 6', { exact: true })).toBeVisible()

  // The admin removes it, answering the one question asked.
  await row.getByRole('button', { name: `Remove table ${label}` }).click()
  const confirm = admin.getByRole('dialog', { name: `Remove table ${label}?` })
  await confirm.getByRole('button', { name: /^remove table$/i }).click()
  await expect(confirm).toBeHidden()

  // Off the admin's working list and into Archived, at once.
  const archived = admin.getByRole('region', { name: /^archived$/i })
  await expect(archived.getByText(label, { exact: true })).toBeVisible()

  // The second claim. Gone from the waiter's floor, with nobody touching it.
  await expect(onWaiterFloor).toBeHidden()
})
