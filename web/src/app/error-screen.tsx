import { useTranslation } from 'react-i18next'
import { isRouteErrorResponse, useRouteError } from 'react-router'

import { apiErrorMessage } from '@/shared/api/error-message'
import { Button } from '@/shared/ui/button'

/**
 * What a route renders when it throws.
 *
 * It sits outside `SurfaceShell`, because the shell is one of the things that
 * may have thrown, so it paints its own background and column.
 *
 * The sentence under the heading comes from the API's stable error code where
 * there is one, mapped to a translated key. The API's own English `message` is
 * never rendered: it is written for a log, and one English sentence in the
 * middle of a Hindi screen is exactly what this feature exists to prevent. In
 * development the raw detail is dumped below, which is a debugging aid rather
 * than something a user is ever shown.
 */
export function ErrorScreen() {
  const { t } = useTranslation()
  const error = useRouteError()

  return (
    <div className="min-h-screen bg-background px-4 py-16">
      <div className="shell-width" role="alert">
        <h1 className="text-2xl font-semibold text-foreground">{t('error.title')}</h1>

        <p className="mt-2 text-sm text-muted-foreground">
          {apiErrorMessage(isRouteErrorResponse(error) ? error.data : undefined, t)}
        </p>

        {import.meta.env.DEV && (
          <pre className="mt-4 overflow-x-auto rounded-md border-line border-border bg-card p-4 text-sm text-card-foreground">
            {error instanceof Error ? error.message : String(error)}
          </pre>
        )}

        <Button
          variant="secondary"
          className="mt-6"
          onClick={() => {
            window.location.reload()
          }}
        >
          {t('error.retry')}
        </Button>
      </div>
    </div>
  )
}
