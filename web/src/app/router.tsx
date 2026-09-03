import {
  createBrowserRouter,
  redirect,
  type LoaderFunctionArgs,
  type RouteObject,
} from 'react-router'

import { AdminHome } from '@/admin/routes/admin-home'
import { RestaurantSettings } from '@/admin/routes/restaurant-settings'
import { KitchenHome } from '@/kitchen/routes/kitchen-home'
import { identityQuery, type Identity, type Role } from '@/shared/session/identity'
import { landingFor, REGISTER_PATH, signInPathFor, SIGN_IN_PATH } from '@/shared/session/signed-out'
import { WaiterHome } from '@/waiter/routes/waiter-home'

import { queryClient } from './query-client'
import { ErrorScreen } from './error-screen'
import { NotFound } from './not-found'
import { Account } from './routes/account'
import { Register } from './routes/register'
import { SignIn } from './routes/sign-in'
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
 * The gate is a loader rather than a check inside a component, and that is the
 * whole point of it. A loader runs before its element renders, so a signed out
 * visitor to `/admin` never sees the admin screen, not even for the frame it
 * would take a `useEffect` to redirect. The role gates below work the same way.
 *
 * None of this is a security control. Every one of these screens asks the API
 * for its data and the API refuses whatever the browser decided; this is here so
 * a chef with wet hands does not land on a settings form.
 */

/**
 * The signed in identity, or a redirect to the sign in screen.
 *
 * `ensureQueryData` rather than a fetch, so a reload asks the server once and
 * every later navigation reads the answer already in the cache.
 */
async function requireIdentity({ request }: LoaderFunctionArgs): Promise<Identity> {
  const identity = await queryClient.ensureQueryData(identityQuery)

  if (!identity) {
    const url = new URL(request.url)
    goTo(signInPathFor(url.pathname, url.search))
  }

  return identity
}

/**
 * Redirects out of a loader.
 *
 * React Router says a redirect by throwing a `Response`, which is how a loader
 * stops without returning a value. The lint rule that wants only errors thrown
 * is right about ordinary code and wrong about this one framework idiom, so the
 * exception lives here, once, rather than beside every loader.
 */
function goTo(path: string): never {
  // eslint-disable-next-line @typescript-eslint/only-throw-error -- React Router's own way of saying "go here instead", and the only way a loader can.
  throw redirect(path)
}

/** The same, plus: this surface is one that role holds. */
function requireRole(role: Role) {
  return async (args: LoaderFunctionArgs): Promise<Identity> => {
    const identity = await requireIdentity(args)

    if (identity.staff.role !== role) {
      // Sent to the surface they do hold rather than shown a refusal. There is
      // nothing for them to do about it, and their own screen is one tap away.
      goTo(landingFor(identity.staff.role))
    }

    return identity
  }
}

/**
 * Somebody already signed in has no business on the sign in screen.
 *
 * Without this, signing in and then pressing back lands on a form that would
 * sign them in again as the person they already are.
 */
async function redirectIfSignedIn(): Promise<null> {
  const identity = await queryClient.ensureQueryData(identityQuery)

  if (identity) {
    goTo(landingFor(identity.staff.role))
  }

  return null
}

/**
 * The design gallery, in development only.
 *
 * `import.meta.env.DEV` is replaced with the literal `false` in a production
 * build, so the bundler drops this branch and, with it, the dynamic import
 * inside it. The gallery and everything it pulls in are never emitted, which is
 * stronger than hiding the route behind a check at runtime.
 */
const developmentRoutes: RouteObject[] = import.meta.env.DEV
  ? [
      {
        path: 'design',
        lazy: async () => {
          const { DesignGallery } = await import('./design/design-gallery')
          return { Component: DesignGallery }
        },
      },
    ]
  : []

export const router = createBrowserRouter([
  // Outside the shell, and outside the gate. These two are the only screens a
  // signed out visitor ever reaches.
  { path: SIGN_IN_PATH, element: <SignIn />, loader: redirectIfSignedIn },
  { path: REGISTER_PATH, element: <Register />, loader: redirectIfSignedIn },
  {
    // Named, so a screen inside the shell can read the identity the loader
    // already resolved rather than asking for it again.
    id: 'root',
    path: '/',
    element: <RootLayout />,
    loader: requireIdentity,
    errorElement: <ErrorScreen />,
    children: [
      { index: true, element: <SystemStatus /> },
      { path: 'account', element: <Account /> },
      {
        path: 'admin',
        loader: requireRole('admin'),
        children: [
          { index: true, element: <AdminHome /> },
          { path: 'settings', element: <RestaurantSettings /> },
        ],
      },
      {
        path: 'waiter',
        loader: requireRole('waiter'),
        children: [{ index: true, element: <WaiterHome /> }],
      },
      {
        path: 'kitchen',
        loader: requireRole('chef'),
        children: [{ index: true, element: <KitchenHome /> }],
      },
      ...developmentRoutes,
      { path: '*', element: <NotFound /> },
    ],
  },
])
