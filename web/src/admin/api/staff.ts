import { queryOptions } from '@tanstack/react-query'

import { ApiCallError } from '@/shared/api/call-error'
import { api } from '@/shared/api/client'
import type { components } from '@/shared/api/schema'
import { staffKey } from '@/shared/events/query-keys'

/**
 * Everything the admin's staff screen asks the API for.
 *
 * One read and six writes. Every write answers with the row it wrote, and the
 * screen does not put that row into the cache: it invalidates `['staff']` on
 * success and reads the list again. That is spec 0007's rule of no cache
 * optimism kept, and here it matters more than usual, because several of these
 * writes change a second thing the response does not carry (every session that
 * person held) and one of them, the role change, may write nothing at all.
 *
 * Nothing here sends a restaurant. The API reads it from the session, and a
 * staff id from another restaurant is a `404` this screen can do nothing about.
 */

/** One member of staff, as the admin's screen reads them. */
export type StaffMember = components['schemas']['StaffMemberDto']

/** What somebody is allowed to be. */
export type StaffRole = StaffMember['role']

/** The three roles, in the order the form offers them. */
export const STAFF_ROLES: readonly StaffRole[] = ['waiter', 'chef', 'admin']

/** Everybody who works here, active first. */
export const staffQuery = queryOptions({
  queryKey: staffKey,
  queryFn: async (): Promise<StaffMember[]> => {
    const { data, error } = await api.GET('/api/staff')
    if (!data) throw new ApiCallError(error)
    return data.staff
  },
})

/** What the create form collects, exactly as typed. */
export interface NewStaffForm {
  displayName: string
  email: string
  password: string
  role: StaffRole
}

/**
 * Adds somebody, with a password they must replace at their first sign in.
 *
 * The password goes up and never comes back. What the hand over panel shows
 * afterwards is the value the admin submitted, carried as the mutation's variable.
 */
export async function createStaff(form: NewStaffForm): Promise<StaffMember> {
  const { data, error } = await api.POST('/api/staff', {
    body: {
      displayName: form.displayName,
      email: form.email,
      password: form.password,
      role: form.role,
    },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Changes somebody's name, naming the version the form loaded. */
export async function renameStaff(
  staffId: string,
  displayName: string,
  version: number,
): Promise<StaffMember> {
  const { data, error } = await api.PATCH('/api/staff/{id}', {
    params: { path: { id: staffId } },
    body: { displayName, version },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/**
 * Changes somebody's role, naming the version the form loaded.
 *
 * It signs every device of theirs out, so their next request runs under the new
 * role rather than under the one they signed in with.
 */
export async function changeStaffRole(
  staffId: string,
  role: StaffRole,
  version: number,
): Promise<StaffMember> {
  const { data, error } = await api.PUT('/api/staff/{id}/role', {
    params: { path: { id: staffId } },
    body: { role, version },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/**
 * Writes a new password onto somebody's row, and signs their devices out.
 *
 * No version, and nothing comes back. An admin doing this has decided this
 * person needs a new password, and a rename somebody else made meanwhile is no
 * reason to refuse it.
 */
export async function resetStaffPassword(staffId: string, password: string): Promise<void> {
  const { error, response } = await api.POST('/api/staff/{id}/password', {
    params: { path: { id: staffId } },
    body: { password },
  })

  if (!response.ok) throw new ApiCallError(error)
}

/** Switches an account off. Never refused because of anything on the floor. */
export async function deactivateStaff(staffId: string): Promise<StaffMember> {
  const { data, error } = await api.POST('/api/staff/{id}/deactivate', {
    params: { path: { id: staffId } },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Brings a switched off account back, exactly as it was. */
export async function reactivateStaff(staffId: string): Promise<StaffMember> {
  const { data, error } = await api.POST('/api/staff/{id}/reactivate', {
    params: { path: { id: staffId } },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/**
 * How many characters a suggested password is built from.
 *
 * Well past the ten character floor the API holds, because this one is read
 * aloud once and then replaced: it has to survive being said across a kitchen
 * and typed by somebody who has never seen it, and it never has to be
 * remembered.
 */
const SUGGESTED_LENGTH = 16

/**
 * The alphabet a suggested password is drawn from.
 *
 * No `l`, no `1`, no `O`, and no `0`. This password's whole life is being read
 * out loud or written on a note and typed once by somebody else, so the pairs
 * that are indistinguishable in most typefaces are the ones that turn a working
 * password into a support conversation.
 */
const ALPHABET = 'abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789'

/**
 * A strong password for the admin to hand over, generated in this browser.
 *
 * `crypto.getRandomValues` rather than `Math.random`, which is not a random
 * source in any browser and would make a suggested password guessable from
 * another one. Rejection sampling rather than a modulo of the raw byte, because
 * 256 does not divide by the alphabet's length and the modulo would quietly
 * make the first few letters more likely than the rest.
 *
 * Generated here rather than by the server on purpose. The plain password then
 * exists only where it already had to, in the form the admin is typing into,
 * and never in a response body.
 */
export function suggestPassword(): string {
  const ceiling = Math.floor(256 / ALPHABET.length) * ALPHABET.length
  const characters: string[] = []

  while (characters.length < SUGGESTED_LENGTH) {
    const batch = new Uint8Array(SUGGESTED_LENGTH)
    crypto.getRandomValues(batch)

    for (const byte of batch) {
      if (byte >= ceiling) continue
      characters.push(ALPHABET[byte % ALPHABET.length] ?? '')
      if (characters.length === SUGGESTED_LENGTH) break
    }
  }

  return characters.join('')
}
