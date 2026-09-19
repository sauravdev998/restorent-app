import { Outlet, useOutletContext } from 'react-router'

import { useReadyAlerts } from '@/waiter/alerts/use-ready-alerts'
import { ReadyAlert } from '@/waiter/components/ready-alert'

/**
 * The frame around every waiter screen, and the one owner of the ready alert.
 *
 * The alert lives here rather than on any one screen, so a waiter hears about
 * food at table 4 while they are taking an order at table 9, or standing on
 * the floor view (spec 0011, AC-9). It holds one Orders query and one reminder
 * timer, and both stop the moment the waiter leaves the waiter surface,
 * because this component is what unmounts.
 *
 * The live stream's context from the shell is handed straight on, so screens
 * below read it exactly as they did before this frame existed.
 */
export function WaiterLayout() {
  const live = useOutletContext()
  const { pending, acknowledgeAll } = useReadyAlerts()

  return (
    <div className="space-y-4">
      <ReadyAlert dishes={pending} onAcknowledge={acknowledgeAll} />
      <Outlet context={live} />
    </div>
  )
}
