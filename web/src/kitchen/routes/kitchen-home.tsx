import { useTranslation } from 'react-i18next'

/**
 * The kitchen surface's landing screen.
 *
 * Identical markup to the admin and waiter screens, and that is the point: the
 * classes below are the same three on all three screens, yet this one renders
 * at kitchen size because `RootLayout` put `data-surface="kitchen"` on the
 * document. There is no kitchen specific class here and there never should be.
 *
 * Feature 13 builds the real ticket display on top of it.
 */
export function KitchenHome() {
  // Its own namespace, plus `common` for the words the shell shares.
  const { t } = useTranslation(['kitchen', 'common'])

  return (
    <div>
      <h1 className="text-2xl font-semibold text-foreground">{t('title')}</h1>
      <p className="mt-2 text-sm text-muted-foreground">{t('common:placeholder')}</p>
    </div>
  )
}
