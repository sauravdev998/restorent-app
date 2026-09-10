import { describe, expect, it } from 'vitest'

import { clockOffset, onDeviceClock } from './server-clock'

/**
 * Ages measured against the server's clock rather than the device's.
 *
 * The failure this exists to stop is quiet and total: a kitchen tablet whose
 * clock is twenty minutes fast marks every ticket late the moment it appears,
 * and the one number a chef acts on becomes the one number nobody can trust.
 * Nothing on that screen would look broken.
 */

/** A fixed instant, so nothing here depends on when the test runs. */
const SERVER_TIME = '2026-09-08T12:00:00.000Z'
const SERVER_MS = Date.parse(SERVER_TIME)

describe('clockOffset', () => {
  it('is nothing when the two clocks agree', () => {
    expect(clockOffset(SERVER_TIME, SERVER_MS)).toBe(0)
  }) // covers: AC-6

  it('is positive when the device is ahead', () => {
    const twentyMinutes = 20 * 60_000
    expect(clockOffset(SERVER_TIME, SERVER_MS + twentyMinutes)).toBe(twentyMinutes)
  }) // covers: AC-6

  it('is negative when the device is behind', () => {
    expect(clockOffset(SERVER_TIME, SERVER_MS - 90_000)).toBe(-90_000)
  }) // covers: AC-6

  it('corrects nothing rather than wildly when the server time is unreadable', () => {
    // Showing an uncorrected age is a small error. Shifting every ticket by
    // `NaN` would show nothing at all, on the screen that can least afford it.
    expect(clockOffset('not a timestamp', SERVER_MS)).toBe(0)
    expect(clockOffset('', SERVER_MS)).toBe(0)
  }) // covers: AC-6
})

describe('onDeviceClock', () => {
  it('leaves a timestamp alone when the clocks agree', () => {
    expect(Date.parse(onDeviceClock(SERVER_TIME, 0))).toBe(SERVER_MS)
  }) // covers: AC-6

  /**
   * The whole point, stated as the scenario from the acceptance criteria: a
   * ticket sent one minute ago still reads about one minute on a device whose
   * clock is twenty minutes out.
   */
  it('makes a ticket read its true age on a device with a wrong clock', () => {
    const twentyMinutes = 20 * 60_000
    const deviceNow = SERVER_MS + twentyMinutes

    // The ticket was sent one minute before the server built its answer.
    const sentAt = new Date(SERVER_MS - 60_000).toISOString()

    const offset = clockOffset(SERVER_TIME, deviceNow)
    const since = Date.parse(onDeviceClock(sentAt, offset))

    // `ElapsedTime` counts `Date.now() - since`, and on this device `Date.now()`
    // is `deviceNow`.
    expect(deviceNow - since).toBe(60_000)
  }) // covers: AC-6

  it('works the same when the device is behind rather than ahead', () => {
    const deviceNow = SERVER_MS - 7 * 60_000
    const sentAt = new Date(SERVER_MS - 90_000).toISOString()

    const since = Date.parse(onDeviceClock(sentAt, clockOffset(SERVER_TIME, deviceNow)))

    expect(deviceNow - since).toBe(90_000)
  }) // covers: AC-6

  it('hands back an unreadable timestamp untouched', () => {
    expect(onDeviceClock('not a timestamp', 5_000)).toBe('not a timestamp')
  }) // covers: AC-6
})
