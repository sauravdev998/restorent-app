import { beforeEach, describe, expect, it, vi } from 'vitest'

import { IDENTITY_KEY, type Identity } from '@/shared/session/identity'
import { SIGN_IN_PATH } from '@/shared/session/signed-out'

import { queryClient } from './query-client'
import { router } from './router'
import { CHOOSE_PASSWORD_PATH } from './routes/choose-password'

/**
 * The two gates the forced password change added to the route table, spec 0009.
 *
 * Driven through the real router's `navigate`, which runs the loaders and moves
 * the location without rendering a screen. What is under test is where somebody
 * ends up, and that is decided entirely by the loaders.
 *
 * The client is replaced so the router's first navigation, made the moment the
 * module loads, reads a signed out answer rather than reaching for a network.
 */
vi.mock('@/shared/api/client', () => ({
  api: {
    GET: vi.fn(() => Promise.resolve({ response: new Response(null, { status: 401 }) })),
    POST: vi.fn(),
    PUT: vi.fn(),
    PATCH: vi.fn(),
  },
  handleSignedOut: vi.fn(),
}))

function signedInAs(role: Identity['staff']['role'], mustChangePassword: boolean): Identity {
  return {
    staff: {
      id: '00000000-0000-7000-8000-000000000011',
      displayName: 'Bo Waiter',
      email: 'bo@example.test',
      role,
      language: null,
      mustChangePassword,
    },
    restaurant: {
      id: '00000000-0000-7000-8000-000000000002',
      name: 'The Test Kitchen',
      address: null,
      countryCode: 'IN',
      currencyCode: 'INR',
      currencyDecimals: 2,
      timezone: 'Asia/Kolkata',
      defaultLanguage: 'en',
      formattingLocale: 'en-IN',
    },
  }
}

/** Where the router settles after asking for `path`. */
async function landingFrom(path: string, identity: Identity | null): Promise<string> {
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, identity)
  await router.navigate(path)
  return router.state.location.pathname
}

beforeEach(() => {
  queryClient.clear()
})

describe('the forced password change gates', () => {
  it('sends somebody who owes a password to the change screen from anywhere in the shell', async () => {
    const owing = signedInAs('waiter', true)

    expect(await landingFrom('/waiter', owing)).toBe(CHOOSE_PASSWORD_PATH)
    expect(await landingFrom('/account', owing)).toBe(CHOOSE_PASSWORD_PATH)
    expect(await landingFrom('/', owing)).toBe(CHOOSE_PASSWORD_PATH)
  }) // covers: AC-4, AC-21

  it('lets somebody who owes a password stay on the change screen', async () => {
    expect(await landingFrom(CHOOSE_PASSWORD_PATH, signedInAs('chef', true))).toBe(
      CHOOSE_PASSWORD_PATH,
    )
  }) // covers: AC-21

  it('sends somebody who owes nothing from the change screen to their own surface', async () => {
    expect(await landingFrom(CHOOSE_PASSWORD_PATH, signedInAs('chef', false))).toBe('/kitchen')
    expect(await landingFrom(CHOOSE_PASSWORD_PATH, signedInAs('waiter', false))).toBe('/waiter')
  }) // covers: AC-21

  it('sends a signed out visitor from the change screen to sign in', async () => {
    expect(await landingFrom(CHOOSE_PASSWORD_PATH, null)).toBe(SIGN_IN_PATH)
  }) // covers: AC-21

  it('leaves somebody who owes nothing where they asked to go', async () => {
    expect(await landingFrom('/waiter', signedInAs('waiter', false))).toBe('/waiter')
  }) // covers: AC-4
})
