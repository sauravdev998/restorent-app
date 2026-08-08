import { useTranslation } from 'react-i18next'
import { useRouteError } from 'react-router'

/** What a route renders when it throws. Feature 3 sends these to monitoring. */
export function ErrorScreen() {
  const { t } = useTranslation()
  const error = useRouteError()

  return (
    <div className="mx-auto max-w-4xl px-4 py-16" role="alert">
      <h1 className="text-xl font-semibold">{t('error.title')}</h1>
      {import.meta.env.DEV && (
        <pre className="mt-4 overflow-x-auto rounded bg-slate-100 p-4 text-sm dark:bg-slate-900">
          {error instanceof Error ? error.message : String(error)}
        </pre>
      )}
      <button
        type="button"
        onClick={() => window.location.reload()}
        className="mt-6 min-h-11 rounded border border-slate-300 px-4 py-2 dark:border-slate-700"
      >
        {t('error.retry')}
      </button>
    </div>
  )
}
