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

/**
 * Spec 0008 AC-4: a dish reordered with the keyboard alone, and every step of
 * it heard by a screen reader in the admin's own language.
 *
 * No pointer anywhere in it: the handle is focused, picked up with Space,
 * moved with an arrow key, and dropped with Space. The words are read from
 * dnd-kit's own live region, which is what a screen reader is listening to.
 * The move is put back at the end, so the menu is left as it was found.
 */
test('an admin reorders a dish with the keyboard alone, and hears it in their language', async ({
  browser,
}) => {
  const admin = await signIn(browser, ADMIN)
  await admin.getByRole('link', { name: /^menu$/i }).click()
  await expect(admin.getByRole('heading', { level: 1, name: /^menu$/i })).toBeVisible()

  // The first category on the menu, whatever it is called, and its dishes'
  // handles, leaving out the category's own handle.
  const section = admin.getByRole('region').first()
  const handles = section.getByRole('button', { name: /^Move (?!the )/ })
  await expect(handles.nth(1)).toBeVisible()

  const labels = await handles.evaluateAll((buttons) =>
    buttons.map((button) => button.getAttribute('aria-label') ?? ''),
  )
  const total = labels.length
  const second = (labels[1] ?? '').replace(/^Move /, '')
  // Every sortable list on the page has its own live region, and only the one
  // whose item is being moved says anything.
  const heard = admin.locator('[id^="DndLiveRegion"]').filter({ hasText: /\S/ })

  /** The order the server holds, read through the admin's own session. */
  const firstOnServer = () =>
    admin.evaluate(async (index) => {
      const response = await fetch('/api/admin/menu')
      const menu = (await response.json()) as {
        categories: { dishes: { name: string }[] }[]
      }
      return menu.categories[index]?.dishes[0]?.name ?? ''
    }, 0)

  // Pick up the second dish, move it up one, and drop it.
  await handles.nth(1).focus()
  await admin.keyboard.press('Space')
  await expect(heard).toHaveText(`Picked up ${second}. It is in position 2 of ${String(total)}.`)
  await admin.keyboard.press('ArrowUp')
  await expect(heard).toHaveText(`${second} moved to position 1 of ${String(total)}.`)
  await admin.keyboard.press('Space')
  await expect(heard).toHaveText(`${second} dropped in position 1 of ${String(total)}.`)

  await expect.poll(firstOnServer).toBe(second)

  // Put it back the same way, once the screen shows the server's order.
  const moved = section.getByRole('button', { name: `Move ${second}`, exact: true })
  await expect(handles.first()).toHaveAccessibleName(`Move ${second}`)
  await moved.focus()
  await admin.keyboard.press('Space')
  await expect(heard).toHaveText(`Picked up ${second}. It is in position 1 of ${String(total)}.`)
  await admin.keyboard.press('ArrowDown')
  await expect(heard).toHaveText(`${second} moved to position 2 of ${String(total)}.`)
  await admin.keyboard.press('Space')
  await expect(heard).toHaveText(`${second} dropped in position 2 of ${String(total)}.`)
  await expect.poll(firstOnServer).not.toBe(second)

  // The same words, in Hindi, once the admin reads the screen in Hindi.
  // Escape puts the dish back without saving anything.
  await admin.getByRole('combobox', { name: 'Language' }).selectOption('hi')
  const hindiHandle = section.getByRole('button', { name: `${second} को खिसकाएँ`, exact: true })
  await expect(hindiHandle).toBeVisible()

  await hindiHandle.focus()
  await admin.keyboard.press('Space')
  await expect(heard).toHaveText(`${second} उठाया गया। यह ${String(total)} में से स्थान 2 पर है।`)
  await admin.keyboard.press('Escape')
  await expect(heard).toHaveText(
    `खिसकाना रद्द किया गया। ${second} वापस ${String(total)} में से स्थान 2 पर है।`,
  )

  await admin.close()
})
