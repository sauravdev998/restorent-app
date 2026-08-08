import { useTranslation } from 'react-i18next'

/**
 * The waiter surface's landing screen.
 *
 * Empty on purpose. Slice 1 puts the thin order thread here, slice 3 thickens
 * it into the real working screen.
 */
export function WaiterHome() {
  const { t } = useTranslation()

  return (
    <div>
      <h1 className="text-xl font-semibold">{t('surface.waiter')}</h1>
      <p className="mt-2 text-slate-500">{t('surface.placeholder')}</p>
    </div>
  )
}
