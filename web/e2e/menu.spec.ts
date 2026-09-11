import { expect, test, type Browser, type Page } from '@playwright/test'

/**
 * The menu's one live claim, checked the only way it can be: three people on
 * three screens at once.
 *
 * An admin adds a dish and the waiter's ordering screen shows it without being
 * touched. A chef switches it off from the kitchen's Menu tab and the waiter's
 * screen greys it without being touched either. Spec 0008 AC-20, and the
 * scope's own promise for feature 9: marking a dish unavailable stops waiters
 * ordering it at once.
 *
 * **No reload anywhere, and that is the assertion**, for the same reason as the
 * order thread's scenario: a build where the event stream never reaches the
 * browser must not be able to pass by refetching on navigation. Every page here
 * navigates only the way a person would, by following a link or a button.
 *
 * It needs a seeded database and a running API. The dish it adds has a name no
 * earlier run can have used, so a database that has seen this run before still
 * gives it a clean dish to watch.
 */

const ADMIN = { email: 'admin@example.test', password: 'development-only-password' }
const WAITER = { email: 'waiter@example.test', password: 'development-only-password' }
const CHEF = { email: 'chef@example.test', password: 'development-only-password' }

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

/**
 * Puts the waiter on a table's ordering screen.
 *
 * An already occupied table is reused when there is one, so this scenario
 * never takes more than one of the seeded four and never starves the order
 * thread's scenario of a free table.
 */
async function openATable(waiter: Page): Promise<void> {
  await expect(waiter.getByRole('heading', { name: /floor/i })).toBeVisible()

  const view = waiter.getByRole('button', { name: /^view$/i }).first()
  const open = waiter.getByRole('button', { name: /^open table$/i }).first()

  await expect(view.or(open).first()).toBeVisible()

  if (await view.isVisible()) await view.click()
  else await open.click()

  await expect(waiter.getByRole('heading', { level: 1, name: /^table /i })).toBeVisible()
}

test('a dish the admin adds reaches the waiter live, and so does the chef switching it off', async ({
  browser,
}) => {
  const dish = `E2E dish ${String(Date.now())}`

  const admin = await signIn(browser, ADMIN)
  const chef = await signIn(browser, CHEF)
  const waiter = await signIn(browser, WAITER)

  // The waiter's ordering screen opens first and stays open from here on.
  // Nothing below reloads it, so anything that appears on it arrived over the
  // stream.
  await openATable(waiter)
  const onWaiterScreen = waiter.getByText(dish, { exact: true })
  await expect(onWaiterScreen).toBeHidden()

  // The admin walks to the menu the way an admin would.
  await admin.getByRole('link', { name: /^menu$/i }).click()
  await expect(admin.getByRole('heading', { level: 1, name: /^menu$/i })).toBeVisible()

  await admin.getByRole('button', { name: /^add a dish$/i }).click()
  const form = admin.getByRole('dialog')
  await form.getByLabel(/^name/i).fill(dish)
  await form.getByLabel(/^price/i).fill('150')
  await form.getByRole('button', { name: /^add dish$/i }).click()
  await expect(form).toBeHidden()

  // On the admin's own screen at once, from the refetch its save triggered.
  await expect(admin.getByText(dish, { exact: true })).toBeVisible()

  // The first claim. No reload, no click, no navigation on the waiter's page.
  await expect(onWaiterScreen).toBeVisible()
  const addOne = waiter.getByRole('button', { name: `Add one ${dish}` })
  await expect(addOne).not.toHaveAttribute('aria-disabled', 'true')

  // The chef walks to the kitchen's Menu tab and switches the new dish off.
  await chef.getByRole('link', { name: /^menu$/i }).click()
  await expect(chef.getByRole('heading', { level: 1, name: /^menu$/i })).toBeVisible()

  const theSwitch = chef.getByRole('switch', { name: `${dish} available` })
  await expect(theSwitch).toHaveAttribute('aria-checked', 'true')
  await theSwitch.click()
  await expect(theSwitch).toHaveAttribute('aria-checked', 'false')

  // The second claim. The waiter's screen greys the dish and refuses to add
  // it, with nobody touching that screen.
  await expect(addOne).toHaveAttribute('aria-disabled', 'true')

  // And the admin's screen, which nobody has touched since the save, shows it
  // off as well, because the chef's switch is an event like any other.
  await expect(admin.getByRole('switch', { name: `${dish} available` })).toHaveAttribute(
    'aria-checked',
    'false',
  )

  // The admin removes it. It lands in Archived on the admin's screen at once,
  // and leaves the waiter's screen with nobody touching that screen, which is
  // also what stops a database that has seen many runs growing a long menu.
  await admin.getByRole('button', { name: `Remove ${dish}` }).click()
  await admin
    .getByRole('dialog')
    .getByRole('button', { name: /^remove dish$/i })
    .click()
  await expect(admin.getByRole('dialog')).toBeHidden()
  await expect(admin.getByRole('button', { name: `Put back ${dish}` })).toBeVisible()

  await expect(onWaiterScreen).toBeHidden()

  await admin.close()
  await chef.close()
  await waiter.close()
})
