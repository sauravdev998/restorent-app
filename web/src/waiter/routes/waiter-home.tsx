import { useTranslation } from 'react-i18next'

/**
 * The waiter surface's landing screen.
 *
 * Empty on purpose. Slice 1 puts the thin order thread here, slice 3 thickens
 * it into the real working screen.
 */
export function WaiterHome() {
  // Its own namespace, plus `common` for the words the shell shares.
  const { t } = useTranslation(['waiter', 'common'])

  return (
    <div>
      <h1 className="text-2xl font-semibold text-foreground">{t('title')}</h1>
      <p className="mt-2 text-sm text-muted-foreground">{t('common:placeholder')}</p>
    </div>
  )
}
