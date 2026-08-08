import { useTranslation } from 'react-i18next'

/**
 * The kitchen surface's landing screen.
 *
 * Empty on purpose, but already using the kitchen type scale, because this
 * screen is read from across a hot kitchen rather than from a desk. Feature 5
 * sets the real design for it and feature 13 builds the ticket display.
 */
export function KitchenHome() {
  const { t } = useTranslation()

  return (
    <div>
      <h1 className="text-kitchen font-semibold">{t('surface.kitchen')}</h1>
      <p className="mt-2 text-slate-500">{t('surface.placeholder')}</p>
    </div>
  )
}
