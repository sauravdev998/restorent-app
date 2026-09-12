import { expect, test, type Browser, type BrowserContextOptions, type Page } from '@playwright/test'

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

/**
 * Signs a person in on their own context, so each holds its own cookie.
 *
 * `options` is for the one scenario that needs a different kind of device: a
 * phone with a touchscreen. Everything else takes the default desktop context.
 */
async function signIn(
  browser: Browser,
  who: { email: string; password: string },
  options: BrowserContextOptions = {},
): Promise<Page> {
  const context = await browser.newContext(options)
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

/**
 * Spec 0008 AC-4, the third way in: a finger.
 *
 * The handle carries a long press before it drags, `TouchSensor` with a 250ms
 * delay and a 6px tolerance, and that delay is the whole feature. Without it a
 * thumb scrolling a long menu picks a dish up instead of moving the page, which
 * is how a menu gets silently reordered in somebody's pocket. So this checks
 * both halves: a quick swipe over the handle does nothing at all, and a press
 * held past the delay drags and saves.
 *
 * Touch has to be driven through the devtools protocol. Playwright's own
 * touchscreen only taps, and a tap cannot express "hold, then move", which is
 * exactly the distinction under test. Real timing matters here, so the waits
 * are real waits rather than anything faked.
 */
test('an admin reorders a dish with a finger, and a quick swipe over the handle does not', async ({
  browser,
}) => {
  const admin = await signIn(browser, ADMIN, {
    viewport: { width: 390, height: 844 },
    hasTouch: true,
    isMobile: true,
  })

  await admin.getByRole('link', { name: /^menu$/i }).click()
  await expect(admin.getByRole('heading', { level: 1, name: /^menu$/i })).toBeVisible()

  const section = admin.getByRole('region').first()
  const handles = section.getByRole('button', { name: /^Move (?!the )/ })
  await expect(handles.nth(1)).toBeVisible()

  const labels = await handles.evaluateAll((buttons) =>
    buttons.map((button) => button.getAttribute('aria-label') ?? ''),
  )
  const total = labels.length
  const last = (labels[total - 1] ?? '').replace(/^Move /, '')
  const heard = admin.locator('[id^="DndLiveRegion"]').filter({ hasText: /\S/ })

  /** The first category the server holds, with its dishes in its own order. */
  const onServer = () =>
    admin.evaluate(async () => {
      const response = await fetch('/api/admin/menu')
      const menu = (await response.json()) as {
        categories: { id: string; dishes: { id: string; name: string }[] }[]
      }
      const category = menu.categories[0]
      return {
        id: category?.id ?? '',
        names: (category?.dishes ?? []).map((dish) => dish.name),
        ids: (category?.dishes ?? []).map((dish) => dish.id),
      }
    })

  const before = await onServer()
  const namesOnServer = async () => (await onServer()).names

  const cdp = await admin.context().newCDPSession(admin)
  const finger = (type: 'touchStart' | 'touchMove' | 'touchEnd', at?: { x: number; y: number }) =>
    cdp.send('Input.dispatchTouchEvent', { type, touchPoints: at ? [at] : [] })

  /** The middle of a handle, which is where a finger would land on it. */
  const middleOf = async (label: string) => {
    const handle = section.getByRole('button', { name: label, exact: true })
    await handle.scrollIntoViewIfNeeded()
    const at = await handle.boundingBox()
    if (!at) throw new Error(`the handle ${label} has no box to press`)
    return { x: Math.round(at.x + at.width / 2), y: Math.round(at.y + at.height / 2) }
  }

  const from = await middleOf(`Move ${last}`)
  const to = await middleOf((labels[0] ?? '').replace(/^Move /, '').replace(/^/, 'Move '))

  // A quick swipe: down, away within the delay, up. A thumb scrolling past.
  await finger('touchStart', from)
  await admin.waitForTimeout(60)
  await finger('touchMove', { x: from.x, y: from.y + 45 })
  await admin.waitForTimeout(120)
  await finger('touchEnd')
  await admin.waitForTimeout(600)

  // Nothing was picked up, so nothing was said and nothing moved.
  await expect(heard).toHaveCount(0)
  expect(await namesOnServer()).toEqual(before.names)

  // The same handle, held past the delay first, then moved the same way.
  await finger('touchStart', from)
  await admin.waitForTimeout(420)
  await expect(heard).toHaveText(
    `Picked up ${last}. It is in position ${String(total)} of ${String(total)}.`,
  )

  for (let step = 1; step <= 10; step += 1) {
    await finger('touchMove', { x: from.x, y: Math.round(from.y + ((to.y - from.y) * step) / 10) })
    await admin.waitForTimeout(35)
  }
  await expect(heard).toHaveText(`${last} moved to position 1 of ${String(total)}.`)

  await finger('touchEnd')
  await expect(heard).toHaveText(`${last} dropped in position 1 of ${String(total)}.`)

  // And the drop reached the server, which is the half a live region cannot
  // tell you: an announcement is made before the save is even sent.
  await expect.poll(async () => (await namesOnServer())[0]).toBe(last)

  // Put the menu back as it was found. This is tidying up rather than part of
  // the claim, so it goes straight to the endpoint instead of through a second
  // drag: a cleanup that can itself flake is worse than no cleanup.
  await admin.evaluate(
    async (original: { id: string; ids: string[] }) =>
      void (await fetch(`/api/admin/menu/categories/${original.id}/dish-order`, {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ ids: original.ids }),
      })),
    { id: before.id, ids: before.ids },
  )
  await expect.poll(namesOnServer).toEqual(before.names)

  await admin.close()
})

/**
 * Spec 0008 AC-5: a drop whose list is no longer the live one is refused whole.
 *
 * A reorder is sent as the complete list of ids. If a dish was added to that
 * category while the admin was mid drag, the list in their hand is one short,
 * and the server answers `409 menu_changed` rather than saving an order that
 * silently drops the new dish. The screen then has to do two things: say why,
 * in the reader's own language, and show the server's order rather than the
 * stale one it was holding.
 *
 * The admin's own refetch is held back for a moment so the race is the same
 * every run. That is the real sequence, an admin who drops before their screen
 * has caught up, made repeatable: the server still refuses on its own terms and
 * the client still recovers on its own. Nothing about the API is faked.
 */
test('a drop made after somebody else changed the category is refused, and the screen catches up', async ({
  browser,
  baseURL,
}) => {
  const admin = await signIn(browser, ADMIN)
  await admin.getByRole('link', { name: /^menu$/i }).click()
  await expect(admin.getByRole('heading', { level: 1, name: /^menu$/i })).toBeVisible()

  const section = admin.getByRole('region').first()
  const handles = section.getByRole('button', { name: /^Move (?!the )/ })
  await expect(handles.nth(1)).toBeVisible()

  const labels = await handles.evaluateAll((buttons) =>
    buttons.map((button) => button.getAttribute('aria-label') ?? ''),
  )
  const total = labels.length
  const last = (labels[total - 1] ?? '').replace(/^Move /, '')
  const heard = admin.locator('[id^="DndLiveRegion"]').filter({ hasText: /\S/ })

  // Somebody else entirely: another admin session, holding its own cookie.
  //
  // `Sec-Fetch-Site` is not decoration. The API refuses any write that cannot
  // prove it came from its own site, and this is the header a browser sends to
  // prove it. Matching on `Origin` instead would not work here: the dev proxy
  // rewrites `Host` on the way through, so the two no longer agree.
  if (!baseURL) throw new Error('this scenario needs a base URL to call the API against')
  const elsewhere = await browser.newContext({
    baseURL,
    extraHTTPHeaders: { 'Sec-Fetch-Site': 'same-origin' },
  })
  await elsewhere.request.post('/api/auth/sign-in', { data: ADMIN })

  const menu = (await (await elsewhere.request.get('/api/admin/menu')).json()) as {
    categories: { id: string; dishes: { id: string; name: string }[] }[]
  }
  const category = menu.categories[0]
  if (!category) throw new Error('the seeded menu has no category to drag inside')

  const middleOf = async (label: string) => {
    const handle = section.getByRole('button', { name: label, exact: true })
    await handle.scrollIntoViewIfNeeded()
    const at = await handle.boundingBox()
    if (!at) throw new Error(`the handle ${label} has no box to grab`)
    return { x: Math.round(at.x + at.width / 2), y: Math.round(at.y + at.height / 2) }
  }
  const from = await middleOf(`Move ${last}`)
  const to = await middleOf(`${labels[0] ?? ''}`)

  // Hold this screen's own refetch back, so the drop below always lands while
  // the screen still believes in the shorter list. Only the read is delayed;
  // the reorder itself goes straight to the API.
  await admin.route('**/api/admin/menu', async (route) => {
    await new Promise((wake) => setTimeout(wake, 3000))
    await route.continue()
  })

  // Pick the last dish up and move it to the top, but do not let go yet.
  await admin.mouse.move(from.x, from.y)
  await admin.mouse.down()
  for (let step = 1; step <= 10; step += 1) {
    await admin.mouse.move(from.x, Math.round(from.y + ((to.y - from.y) * step) / 10), { steps: 2 })
  }
  await expect(heard).toHaveText(`${last} moved to position 1 of ${String(total)}.`)

  // A dish arrives in that very category from the other session.
  const slipped = (await (
    await elsewhere.request.post('/api/admin/menu/dishes', {
      data: {
        categoryId: category.id,
        name: `E2E raced ${String(Date.now())}`,
        price: '12.00',
        diet: 'veg',
      },
    })
  ).json()) as { id: string; name: string }

  await admin.mouse.up()

  // The refusal, in the admin's own words rather than the API's English, and
  // it lands in both places it has to: read out to a screen reader, and on the
  // screen for everybody else.
  const refusal = /The menu changed while you were moving things\./
  await expect(admin.getByRole('status').filter({ hasText: refusal })).toBeAttached()
  await expect(
    admin.getByRole('region', { name: /notifications/i }).getByText(refusal),
  ).toBeVisible()

  // Nothing was saved: the server's order is the one it had, with the new dish
  // at the end where it was created.
  const after = (await (await elsewhere.request.get('/api/admin/menu')).json()) as {
    categories: { id: string; dishes: { name: string }[] }[]
  }
  const saved = (after.categories.find((one) => one.id === category.id)?.dishes ?? []).map(
    (dish) => dish.name,
  )
  expect(saved).toEqual([...category.dishes.map((dish) => dish.name), slipped.name])

  // And the screen shows that order, not the one it was holding mid drag.
  await expect
    .poll(() =>
      section
        .getByRole('button', { name: /^Move (?!the )/ })
        .evaluateAll((buttons) =>
          buttons.map((button) => (button.getAttribute('aria-label') ?? '').replace(/^Move /, '')),
        ),
    )
    .toEqual(saved)

  await elsewhere.request.post(`/api/admin/menu/dishes/${slipped.id}/archive`)
  await elsewhere.close()
  await admin.close()
})

/**
 * Spec 0008 AC-13: the diet marks where colour is taken away from them.
 *
 * The marks are the one thing on the menu that a guest with an allergy relies
 * on, and colour alone cannot carry them. Two places take the colour away: a
 * reader running Windows High Contrast, where every colour collapses to the
 * system palette, and paper. In both the shapes have to keep telling the three
 * apart, and on paper they have to be black rather than a pale tint nobody can
 * see. The switch thumb is checked alongside them because a switch whose thumb
 * vanishes under forced colours cannot be read as on or off.
 *
 * This is the one claim the contrast script cannot make. That script checks
 * token pairs for dark, light and print; it cannot know what a browser does
 * with `forced-colors: active`, which is a real rendering mode with a real
 * cascade, so it takes a real browser to ask.
 */
test('the diet marks keep their shapes under forced colours, and print in black', async ({
  browser,
}) => {
  const admin = await signIn(browser, ADMIN)
  await admin.getByRole('link', { name: /^menu$/i }).click()
  await expect(admin.getByRole('heading', { level: 1, name: /^menu$/i })).toBeVisible()

  // The seeded menu is veg and non veg only, so the third mark has to be put
  // on the page before anything can be read off it. Chosen by value, not by
  // its English word, so the language the run starts in does not matter.
  await admin.getByRole('button', { name: /^add a dish$/i }).click()
  const form = admin.getByRole('dialog')
  await form.getByLabel(/^name/i).fill(`E2E egg dish ${String(Date.now())}`)
  await form.getByLabel(/^price/i).fill('90')
  await form.getByLabel(/^diet/i).selectOption('egg')
  await form.getByRole('button', { name: /^add dish$/i }).click()
  await expect(form).toBeHidden()

  await expect(admin.locator('svg[role="img"]').first()).toBeVisible()

  /** One sample per diet marker: its colour, and a signature of its drawing. */
  const marks = () =>
    admin.evaluate(() =>
      [
        ...new Map(
          [...document.querySelectorAll('svg[role="img"]')].map((mark) => [
            mark.getAttribute('aria-label') ?? '',
            {
              name: mark.getAttribute('aria-label') ?? '',
              colour: getComputedStyle(mark).color,
              shape: [...mark.querySelectorAll('circle,ellipse,polygon,path,rect')]
                .map((part) => part.tagName.toLowerCase())
                .join(' '),
            },
          ]),
        ).values(),
      ].sort((one, two) => one.name.localeCompare(two.name)),
    )

  // On screen the three are three different colours, which is what forced
  // colours and paper are each about to take away.
  const onScreen = await marks()
  expect(onScreen).toHaveLength(3)
  expect(new Set(onScreen.map((mark) => mark.colour)).size).toBe(3)
  const shapes = onScreen.map((mark) => mark.shape)
  expect(new Set(shapes).size).toBe(3)

  await admin.emulateMedia({ forcedColors: 'active' })

  const forced = await marks()
  // Every mark is now the one system text colour...
  expect(new Set(forced.map((mark) => mark.colour)).size).toBe(1)
  // ...and the shapes are untouched, so they are still told apart.
  expect(forced.map((mark) => mark.shape)).toEqual(shapes)

  // The switch thumb has to stay painted, or on and off look the same.
  const thumb = await admin.evaluate(() => {
    const control = document.querySelector('button[role="switch"] span, button[role="switch"] div')
    return control ? getComputedStyle(control).backgroundColor : ''
  })
  expect(thumb).not.toBe('')
  expect(thumb).not.toBe('rgba(0, 0, 0, 0)')

  await admin.emulateMedia({ forcedColors: 'none', media: 'print' })

  // On paper: black marks on a white page, whatever the screen was doing.
  for (const mark of await marks()) expect(mark.colour).toBe('rgb(0, 0, 0)')
  await expect
    .poll(() => admin.evaluate(() => getComputedStyle(document.body).backgroundColor))
    .toBe('rgb(255, 255, 255)')

  await admin.close()
})
