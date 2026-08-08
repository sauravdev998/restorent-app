import { createBrowserRouter } from 'react-router'

import { AdminHome } from '@/admin/routes/admin-home'
import { KitchenHome } from '@/kitchen/routes/kitchen-home'
import { WaiterHome } from '@/waiter/routes/waiter-home'

import { ErrorScreen } from './error-screen'
import { NotFound } from './not-found'
import { RootLayout } from './root-layout'
import { SystemStatus } from './system-status'

/**
 * The route table, in React Router's data mode.
 *
 * Data mode rather than framework mode, because this is a single page app
 * talking to a separate API rather than a full stack React framework.
 *
 * Three route groups, one per surface, because the admin, the waiter, and the
 * kitchen barely share a screen between them even though they share a codebase.
 *
 * Feature 7 puts a role gate in front of each group, on the server as well as
 * here. Until it does, these routes are open, so nothing behind them may assume
 * it knows who is looking.
 */
export const router = createBrowserRouter([
  {
    path: '/',
    element: <RootLayout />,
    errorElement: <ErrorScreen />,
    children: [
      { index: true, element: <SystemStatus /> },
      { path: 'admin', children: [{ index: true, element: <AdminHome /> }] },
      { path: 'waiter', children: [{ index: true, element: <WaiterHome /> }] },
      { path: 'kitchen', children: [{ index: true, element: <KitchenHome /> }] },
      { path: '*', element: <NotFound /> },
    ],
  },
])
