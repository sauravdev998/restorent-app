/** How hard a waiting time is pressing, from the two thresholds around it. */
export type Urgency = 'calm' | 'warning' | 'late'

/**
 * Which of the three bands a waiting time falls in.
 *
 * Its own module rather than sitting beside `ElapsedTime`, because a file that
 * exports a component and a function is a file whose fast refresh stops working,
 * and this rule is worth testing on its own.
 *
 * Late wins over warning wherever the two overlap, so a pair somebody has stored
 * the wrong way round still reads as the more urgent of the two rather than
 * silently as the calmer one. The database holds a check constraint against that
 * pair existing at all; this is what the screen does if one ever does.
 *
 * @param totalSeconds how long it has been waiting.
 * @param warningAfterSeconds when it turns amber, or `undefined` for never.
 * @param lateAfterSeconds when it turns red.
 */
export function urgencyOf(
  totalSeconds: number,
  warningAfterSeconds: number | undefined,
  lateAfterSeconds: number,
): Urgency {
  if (totalSeconds >= lateAfterSeconds) return 'late'
  if (warningAfterSeconds !== undefined && totalSeconds >= warningAfterSeconds) return 'warning'
  return 'calm'
}
