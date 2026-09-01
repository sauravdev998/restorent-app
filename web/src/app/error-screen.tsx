import { useTranslation } from 'react-i18next'
import { useRouteError } from 'react-router'

import { Button } from '@/shared/ui/button'

/**
 * What a route renders when it throws.
 *
 * It sits outside `SurfaceShell`, because the shell is one of the things that
 * may have thrown, so it paints its own background and column.
 */
export function ErrorScreen() {
  const { t } = useTranslation()
  const error = useRouteError()

  return (
    <div className="min-h-screen bg-background px-4 py-16">
      <div className="shell-width" role="alert">
        <h1 className="text-2xl font-semibold text-foreground">{t('error.title')}</h1>

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
