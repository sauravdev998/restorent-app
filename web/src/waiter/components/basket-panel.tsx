import { TriangleAlert } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Field } from '@/shared/ui/field'
import { Icon } from '@/shared/ui/icon'
import { Input } from '@/shared/ui/input'
import { RestaurantText } from '@/shared/ui/restaurant-text'
import {
  addToLine,
  NOTE_MAX_CHARS,
  noteLength,
  noteTooLong,
  removeOne,
  setNote,
  splitOne,
  takeOut,
  type Basket,
} from '@/waiter/basket'

export interface BasketPanelProps {
  basket: Basket
  /** Changes the basket. The caller saves it. */
  onChange: (change: (basket: Basket) => Basket) => void
  /** Dish ids the menu says can no longer be ordered. */
  flagged: readonly string[]
}

/**
 * The unsent basket, one line per dish and note (spec 0011, AC-6).
 *
 * Every line carries its own note, such as "no onions", which reaches the
 * kitchen exactly as typed. Two soups where only one wants no onions is two
 * lines: "One separately" splits one plate off so it can carry a note of its
 * own. The note is counted the way the server counts it, trimmed and in
 * characters rather than bytes, so 140 Hindi letters fit exactly as 140
 * English ones do.
 *
 * A line whose dish went off the menu while the basket was being built is
 * struck through with a way to take it out, and the send stays blocked until
 * it is gone. The announcement is a status, so it is read out the moment it
 * happens whether or not anybody is looking at the phone.
 */
export function BasketPanel({ basket, onChange, flagged }: BasketPanelProps) {
  const { t } = useTranslation('waiter')

  return (
    <Card as="section" aria-labelledby="basket-heading" className="space-y-3">
      <h3 id="basket-heading" className="text-sm font-medium text-muted-foreground">
        {t('basket.title')}
      </h3>

      {flagged.length > 0 && (
        <p role="status" className="flex items-start gap-2 text-sm font-medium text-status-late">
          <Icon icon={TriangleAlert} size="sm" className="mt-1" />
          {t('basket.flagged', { count: flagged.length })}
        </p>
      )}

      <ul className="divide-y divide-border">
        {basket.lines.map((line, index) => {
          const off = flagged.includes(line.dishId)
          const long = noteTooLong(line.note)

          return (
            <li key={`${line.dishId}-${String(index)}`} className="space-y-2 py-3">
              <div className="flex items-center justify-between gap-3">
                <p className={off ? 'min-w-0 text-muted-foreground line-through' : 'min-w-0'}>
                  <span className="tabular">{line.quantity}</span> ×{' '}
                  <RestaurantText>{line.name}</RestaurantText>
                </p>

                {off ? (
                  <Button
                    variant="secondary"
                    size="sm"
                    aria-label={t('basket.takeOutNamed', { dish: line.name })}
                    onClick={() => {
                      onChange((current) => takeOut(current, line.dishId))
                    }}
                  >
                    {t('basket.takeOut')}
                  </Button>
                ) : (
                  <div className="flex items-center gap-2">
                    <Button
                      variant="secondary"
                      size="sm"
                      aria-label={t('basket.removeOneNamed', { dish: line.name })}
                      onClick={() => {
                        onChange((current) => removeOne(current, index))
                      }}
                    >
                      −
                    </Button>
                    <Button
                      variant="secondary"
                      size="sm"
                      aria-label={t('basket.addOneNamed', { dish: line.name })}
                      onClick={() => {
                        onChange((current) => addToLine(current, index))
                      }}
                    >
                      +
                    </Button>
                  </div>
                )}
              </div>

              {off ? (
                <p className="text-xs font-medium text-status-late">{t('basket.lineOff')}</p>
              ) : (
                <div className="space-y-2">
                  <Field
                    label={t('basket.noteLabel', { dish: line.name })}
                    hint={t('basket.noteCount', {
                      count: noteLength(line.note),
                      max: NOTE_MAX_CHARS,
                    })}
                    {...(long ? { error: t('basket.noteTooLong', { max: NOTE_MAX_CHARS }) } : {})}
                  >
                    <Input
                      value={line.note}
                      placeholder={t('basket.notePlaceholder')}
                      autoComplete="off"
                      onChange={(event) => {
                        const note = event.target.value
                        onChange((current) => setNote(current, index, note))
                      }}
                    />
                  </Field>

                  {line.quantity > 1 && (
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-label={t('basket.splitNamed', { dish: line.name })}
                      onClick={() => {
                        onChange((current) => splitOne(current, index))
                      }}
                    >
                      {t('basket.split')}
                    </Button>
                  )}
                </div>
              )}
            </li>
          )
        })}
      </ul>
    </Card>
  )
}
