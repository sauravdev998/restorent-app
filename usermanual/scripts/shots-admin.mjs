import { chromium } from '../../web/node_modules/@playwright/test/index.mjs'

const BASE = 'http://localhost:5173'
const OUT = new URL('../images', import.meta.url).pathname
const PASS = 'development-only-password'

const browser = await chromium.launch()
const shot = async (page, name, opts = {}) => {
  await page.waitForTimeout(600)
  await page.evaluate(() => document.activeElement?.blur?.())
  await page.screenshot({ path: `${OUT}/${name}.png`, ...opts })
  console.log('shot', name)
}
const step = async (label, fn) => {
  try { await fn() } catch (e) { console.log('FAILED', label, e.message.split('\n')[0]) }
}

const desktop = await browser.newContext({ viewport: { width: 1366, height: 860 }, deviceScaleFactor: 2 })
const page = await desktop.newPage()
page.setDefaultTimeout(15000)

await step('sign in page', async () => {
  await page.goto(`${BASE}/sign-in`)
  await page.getByLabel('Email address').waitFor()
  await shot(page, '01-sign-in')
})
await step('register page', async () => {
  await page.getByRole('link', { name: 'Register a restaurant' }).click()
  await page.getByLabel('Restaurant name').waitFor()
  await page.getByLabel('Restaurant name').fill('Spice Garden')
  await page.getByLabel('Your name').fill('Anita Sharma')
  await shot(page, '02-register')
})

await step('admin sign in', async () => {
  await page.goto(`${BASE}/sign-in`)
  await page.getByLabel('Email address').fill('anita@spicegarden.test')
  await page.getByLabel('Password').fill(PASS)
  await page.getByRole('button', { name: 'Sign in' }).click()
  await page.waitForURL(/admin/)
  console.log('admin landed at', page.url())
})

await step('admin menu', async () => {
  await page.goto(`${BASE}/admin/menu`)
  await page.getByRole('heading', { name: 'Menu', level: 1 }).waitFor()
  await shot(page, '10-admin-menu', { fullPage: true })
})
await step('add dish dialog', async () => {
  await page.getByRole('button', { name: 'Add a dish' }).first().click()
  const dlg = page.getByRole('dialog')
  await dlg.waitFor()
  await dlg.getByLabel('Name').fill('Masala dosa')
  await dlg.getByLabel('Description').fill('Crisp rice crepe with spiced potato, sambar and chutney')
  await dlg.getByLabel('Price').fill('180')
  await shot(page, '11-admin-add-dish')
  await dlg.getByRole('button', { name: 'Add dish' }).click()
  await dlg.waitFor({ state: 'hidden' })
})
await step('add category dialog', async () => {
  await page.getByRole('button', { name: 'Add a category' }).first().click()
  const dlg = page.getByRole('dialog')
  await dlg.getByLabel('Name').fill('Desserts')
  await shot(page, '12-admin-add-category')
  await dlg.getByRole('button', { name: 'Cancel' }).click()
})

await step('admin floor', async () => {
  await page.goto(`${BASE}/admin/floor`)
  await page.getByRole('heading', { level: 1 }).waitFor()
  await shot(page, '13-admin-floor', { fullPage: true })
})
await step('add table dialog', async () => {
  await page.getByRole('button', { name: /add a table/i }).first().click()
  const dlg = page.getByRole('dialog')
  await dlg.waitFor()
  await shot(page, '14-admin-add-table')
  await page.keyboard.press('Escape')
})


await step('settings', async () => {
  await page.goto(`${BASE}/admin/settings`)
  await page.getByRole('heading', { name: 'Restaurant settings' }).waitFor()
  await shot(page, '18-admin-settings', { fullPage: true })
})
await step('account', async () => {
  await page.goto(`${BASE}/account`)
  await page.getByRole('heading', { name: 'Your account' }).waitFor()
  await shot(page, '19-account', { fullPage: true })
})

await browser.close()
