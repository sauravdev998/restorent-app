import { useTranslation } from 'react-i18next'

/** Shown for an address that matches no screen. */
export function NotFound() {
  const { t } = useTranslation()

  return (
    <div>
      <h1 className="text-2xl font-semibold text-foreground">{t('notFound.title')}</h1>
      <p className="mt-2 text-sm text-muted-foreground">{t('notFound.body')}</p>
    </div>
  )
}
