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
      <h1 className="text-2xl font-semibold text-foreground">{t('surface.waiter')}</h1>
      <p className="mt-2 text-sm text-muted-foreground">{t('surface.placeholder')}</p>
    </div>
  )
}
