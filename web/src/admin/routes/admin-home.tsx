import { useTranslation } from 'react-i18next'

/**
 * The admin surface's landing screen.
 *
 * Empty on purpose. The route group exists so slices 2 and 5 have somewhere to
 * land, and so the three surfaces are separated from the first commit rather
 * than being untangled later.
 */
export function AdminHome() {
  const { t } = useTranslation()

  return (
    <div>
      <h1 className="text-xl font-semibold">{t('surface.admin')}</h1>
      <p className="mt-2 text-slate-500">{t('surface.placeholder')}</p>
    </div>
  )
}
