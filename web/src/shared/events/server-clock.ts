/**
 * Ages measured against the server's clock, not the device's.
 *
 * A kitchen tablet is a cheap appliance that lives on a shelf and gets unplugged
 * at closing. Its clock drifts, and nobody notices, because nothing else on it
 * shows the time. The one number a chef acts on is how long a ticket has been
 * waiting, and that number is a subtraction between two clocks: the one the
 * ticket was stamped by, which is the database's, and the one the screen reads,
 * which is the tablet's. A device twenty minutes fast marks every ticket late
 * the moment it appears.
 *
 * The fix is arithmetic rather than trust. Every read that carries an age also
 * carries `serverTime`, the instant the answer was built. The difference between
 * that and the device's own clock at the moment the answer arrived is the
 * device's error, and shifting the timestamp by it cancels the error out. What
 * is left is a true duration, whatever the tablet believes the time is.
 *
 * It survives a wrong timezone too, because both sides are instants rather than
 * wall clock readings, and an instant carries no timezone to get wrong.
 */

/**
 * How far ahead of the server this device's clock is, in milliseconds.
 *
 * Positive when the device is fast. Measured at the moment a response arrives,
 * so it includes the request's travel time, which is milliseconds on a
 * restaurant's own wifi and is not worth modelling out.
 *
 * @param serverTime the `serverTime` the response carried.
 * @param receivedAt the device's clock when it arrived, for testing.
 */
export function clockOffset(serverTime: string, receivedAt: number = Date.now()): number {
  const server = Date.parse(serverTime)

  // A server time this device cannot parse means no correction rather than a
  // wild one. Showing an uncorrected age is a small error; shifting every
  // ticket by `NaN` would show nothing at all.
  if (Number.isNaN(server)) return 0

  return receivedAt - server
}

/**
 * A timestamp shifted onto this device's clock, so a duration measured from it
 * comes out true.
 *
 * Hand the result to `ElapsedTime` as its `since`. It counts against
 * `Date.now()`, which is the device's clock, so the timestamp it counts from
 * has to be on that same clock.
 *
 * @param timestamp the `timestamptz` the API returned, as an ISO string.
 * @param offset what {@link clockOffset} measured.
 */
export function onDeviceClock(timestamp: string, offset: number): string {
  const instant = Date.parse(timestamp)
  if (Number.isNaN(instant)) return timestamp

  return new Date(instant + offset).toISOString()
}
