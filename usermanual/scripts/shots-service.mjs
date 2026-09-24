import { execSync } from 'node:child_process'
import { chromium } from '../../web/node_modules/@playwright/test/index.mjs'

const BASE = 'http://localhost:5173'
const OUT = new URL('../images', import.meta.url).pathname
const PASS = 'development-only-password'
const sql = (q) =>
  execSync(`docker exec -i manual-db psql -U restaurant_owner -d restaurant -tA`, {
    input: q,
  }).toString()

const browser = await chromium.launch()
const shot = async (page, name, opts = {}) => {
  await page.waitForTimeout(800)
  await page.evaluate(() => document.activeElement?.blur?.())
  await page.screenshot({ path: `${OUT}/${name}.png`, ...opts })
  console.log('shot', name)
}
const step = async (label, fn) => {
  try {
    await fn()
  } catch (e) {
    console.log('FAILED', label, e.message.split('\n')[0])
  }
}
const signIn = async (page, email, password = PASS) => {
  await page.goto(`${BASE}/sign-in`)
  await page.getByLabel('Email address').fill(email)
  await page.getByLabel('Password').fill(password)
  await page.getByRole('button', { name: 'Sign in' }).click()
  await page.waitForURL((u) => !u.pathname.startsWith('/sign-in'))
}

// ---- admin: staff screens again, with a known first password
const admin = await (
  await browser.newContext({ viewport: { width: 1366, height: 860 }, deviceScaleFactor: 2 })
).newPage()
admin.setDefaultTimeout(20000)
await step('admin staff', async () => {
  await signIn(admin, 'anita@spicegarden.test')
  await admin.goto(`${BASE}/admin/staff`)
  await admin.getByRole('heading', { name: 'Staff', level: 1 }).waitFor()
  await admin.getByRole('button', { name: 'Add somebody' }).first().click()
  const dlg = admin.getByRole('dialog')
  await dlg.getByLabel('Name').fill('Arjun Mehta')
  await dlg.getByLabel('Email address').fill('arjun@spicegarden.test')
  await dlg.getByLabel('First password').fill('Welcome-2026')
  await dlg.getByRole('button', { name: 'Show the password' }).click()
  await dlg.getByLabel('Role').selectOption({ label: 'Chef' })
  await shot(admin, '16-admin-add-staff')
  await dlg.getByRole('button', { name: 'Create the account' }).click()
  await admin.getByText('can sign in now').waitFor()
  await shot(admin, '17-admin-staff-created')
  await admin.keyboard.press('Escape')
  await admin.waitForTimeout(500)
  await shot(admin, '15-admin-staff', { fullPage: true })
})

// ---- a new staff member's first sign in
await step('choose password', async () => {
  const p = await (
    await browser.newContext({ viewport: { width: 1366, height: 860 }, deviceScaleFactor: 2 })
  ).newPage()
  await signIn(p, 'arjun@spicegarden.test', 'Welcome-2026')
  await p.getByRole('heading', { name: 'Choose your own password' }).waitFor()
  await shot(p, '05-choose-password')
})

// ---- waiter on a phone, chef on a kitchen tablet
const waiter = await (
  await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 3,
    hasTouch: true,
    isMobile: true,
  })
).newPage()
const chef = await (
  await browser.newContext({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1.5 })
).newPage()
waiter.setDefaultTimeout(20000)
chef.setDefaultTimeout(20000)

await step('sign in both', async () => {
  await signIn(waiter, 'ravi@spicegarden.test')
  await signIn(chef, 'meera@spicegarden.test')
  console.log('waiter at', waiter.url(), 'chef at', chef.url())
})
await step('kitchen quiet', async () => {
  await chef.getByText('Nothing on the pass').waitFor()
  await shot(chef, '30-kitchen-empty')
  await chef.mouse.click(960, 900) // unlock the chime
})
await step('waiter floor', async () => {
  await waiter.getByRole('button', { name: 'Open table' }).first().waitFor()
  await shot(waiter, '20-waiter-floor')
})

const openTable = async (n) => {
  await waiter.goto(`${BASE}/waiter`)
  const card = waiter
    .locator('li', { hasText: new RegExp(`^\\s*${n}\\b`) })
    .filter({ has: waiter.getByRole('button', { name: 'Open table' }) })
  await waiter.getByRole('button', { name: 'Open table' }).nth(0).waitFor()
  if (await card.count()) await card.first().getByRole('button', { name: 'Open table' }).click()
  else await waiter.getByRole('button', { name: 'Open table' }).first().click()
  await waiter.getByRole('heading', { name: /^Table / }).waitFor()
}
const add = (dish) => waiter.getByRole('button', { name: `Add one ${dish}` }).click()
const send = async () => {
  await waiter.getByRole('button', { name: /^Send \d+ dish/ }).click()
  await waiter
    .getByText(/Round \d/)
    .first()
    .waitFor()
  await waiter.waitForTimeout(800)
}

// Table 1: one round, backdated so the kitchen shows a late ticket
await step('table A (late)', async () => {
  await openTable(1)
  await add('Chicken biryani')
  await add('Tomato soup')
  await send()
  sql(
    `update order_rounds set sent_at = now() - interval '24 minutes', created_at = now() - interval '24 minutes';`,
  )
})
// Table 2: the showcase table
await step('table B basket', async () => {
  await openTable(2)
  await shot(waiter, '21-waiter-table-empty')
  await add('Paneer tikka')
  await add('Dal makhani')
  await add('Butter naan')
  await add('Butter naan')
  await waiter.getByLabel('Note for Paneer tikka').fill('Less spicy, no onions')
  await waiter
    .getByText('Basket, not sent yet')
    .evaluate((e) => e.scrollIntoView({ block: 'start' }))
  await shot(waiter, '22-waiter-basket')
  await send()
  await waiter
    .getByRole('heading', { name: 'Sent to the kitchen' })
    .evaluate((e) => e.scrollIntoView({ block: 'start' }))
  await shot(waiter, '23-waiter-sent')
  sql(
    `update order_rounds set sent_at = now() - interval '12 minutes', created_at = now() - interval '12 minutes' where sequence_no = 1 and sent_at > now() - interval '1 minute';`,
  )
})
// Table 3: a fresh ticket
await step('table C', async () => {
  await openTable(3)
  await add('Dal makhani')
  await add('Butter naan')
  await send()
})

await step('kitchen pass', async () => {
  await chef.reload()
  await chef.getByTestId('kitchen-ticket').nth(2).waitFor()
  await chef.mouse.click(1900, 1000)
  await shot(chef, '31-kitchen-pass', { fullPage: true })
})
await step('kitchen mark one done', async () => {
  await waiter.goto(`${BASE}/waiter/orders`)
  await waiter.getByRole('heading', { name: 'Orders' }).waitFor()
  const ticket = chef.getByTestId('kitchen-ticket').filter({ hasText: 'Paneer tikka' })
  await ticket.getByRole('button', { name: 'Done', exact: true }).first().click()
  await chef.waitForTimeout(1200)
  await shot(chef, '32-kitchen-one-done', { fullPage: true })
  await ticket.getByRole('button', { name: /Mark every cooking dish/ }).click()
  await chef.getByText('Ready to collect').waitFor()
  await chef.waitForTimeout(1500)
  await shot(chef, '33-kitchen-ready', { fullPage: true })
})

await step('waiter ready alert', async () => {
  await waiter
    .getByText(/ready to collect/)
    .first()
    .waitFor()
  await shot(waiter, '24-waiter-ready-alert')
  await waiter.getByRole('button', { name: 'Acknowledge' }).click()
})
await step('waiter orders', async () => {
  await waiter.evaluate(() => window.scrollTo(0, 0))
  await shot(waiter, '25-waiter-orders')
})
await step('void dialog', async () => {
  await waiter.getByText('Table 3').first().click()
  await waiter.getByRole('button', { name: 'Cancel Butter naan' }).click()
  const dlg = waiter.getByRole('dialog')
  await dlg.waitFor()
  await dlg
    .getByLabel('Why')
    .selectOption({ label: 'Guest changed their mind' })
    .catch(() => {})
  await shot(waiter, '28-waiter-cancel-dish')
  await dlg.getByRole('button', { name: 'Keep the dish' }).click()
})
await step('move dialog', async () => {
  await waiter.getByRole('button', { name: 'Move table' }).click()
  await waiter.getByRole('dialog').waitFor()
  await shot(waiter, '29-waiter-move-table')
  await waiter.getByRole('button', { name: 'Stay here' }).click()
})
await step('serve and close', async () => {
  await waiter.goto(`${BASE}/waiter/orders`)
  await waiter.getByText('Table 2').first().click()
  await waiter.getByRole('button', { name: 'Serve all ready' }).click()
  await waiter.getByText('Served').first().waitFor()
  await waiter.waitForTimeout(1000)
  await waiter
    .getByRole('heading', { name: 'Sent to the kitchen' })
    .evaluate((e) => e.scrollIntoView({ block: 'start' }))
  await shot(waiter, '26-waiter-served')
  await waiter.getByRole('button', { name: 'Close the bill' }).click()
  await waiter.getByText(/^Bill \d+/).waitFor()
  await shot(waiter, '27-waiter-bill-closed')
})

await step('kitchen menu', async () => {
  await chef.getByRole('link', { name: 'Menu' }).click()
  await chef.getByText('Switch a dish off').waitFor()
  await shot(chef, '34-kitchen-menu')
})

await browser.close()
