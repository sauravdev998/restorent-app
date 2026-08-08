import { useTranslation } from 'react-i18next'

/** Shown for an address that matches no screen. */
export function NotFound() {
  const { t } = useTranslation()

  return (
    <div>
      <h1 className="text-xl font-semibold">{t('notFound.title')}</h1>
      <p className="mt-2 text-slate-500">{t('notFound.body')}</p>
    </div>
  )
}
