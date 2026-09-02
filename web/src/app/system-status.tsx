import { useMutation, useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useOutletContext } from 'react-router'

import { healthQuery } from '@/shared/api/health'
import type { LiveEvents } from '@/shared/events/use-live-events'
import { currentRestaurantId } from '@/shared/session/current-restaurant'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'

/**
 * The scaffold's proof of life.
 *
 * It exercises every layer the platform is built on: the generated client
 * calling the Rust API, TanStack Query holding the answer, and the server sent
 * event stream delivering a change that originated in Postgres. Feature 8
 * replaces it with a waiter sending a real dish to a real kitchen screen.
 */
export function SystemStatus() {
  // The system status screen is admin side, so its words live there.
  const { t } = useTranslation('admin')
  const live = useOutletContext<LiveEvents>()
  const health = useQuery(healthQuery)

  const sendProbe = useMutation({
    mutationFn: async () => {
      // Plain fetch, not the generated client, and on purpose. `/api/dev/notify`
      // exists only in development, so it is deliberately absent from the
      // OpenAPI document and therefore absent from the generated types. Casting
      // it into the typed client would be a lie about the API surface. Every
      // real endpoint goes through `api` in `shared/api/client.ts`.
      const response = await fetch('/api/dev/notify', {
        method: 'POST',
        headers: { 'X-Restaurant-Id': currentRestaurantId() },
      })
      if (!response.ok) throw new Error('Could not send the test event.')
    },
  })

  return (
    <div className="space-y-8">
      <section aria-labelledby="system-heading">
        <h1 id="system-heading" className="text-2xl font-semibold text-foreground">
          {t('system.title')}
        </h1>

        {health.isPending && (
          <p className="mt-2 text-sm text-muted-foreground" role="status">
            {t('system.checking')}
          </p>
        )}

        {health.isError && (
          <p className="mt-2 text-sm text-status-late" role="alert">
            {t('system.unreachable')}
          </p>
        )}

        {health.data && (
          <Card className="mt-4">
            <dl className="grid grid-cols-2 gap-x-6 gap-y-2 text-sm" data-testid="health">
              <dt className="text-muted-foreground">{t('system.database')}</dt>
              <dd>{health.data.database === 'up' ? t('system.up') : t('system.down')}</dd>
              <dt className="text-muted-foreground">{t('system.listener')}</dt>
              <dd>{health.data.listener === 'up' ? t('system.up') : t('system.down')}</dd>
            </dl>
          </Card>
        )}
      </section>

      <section aria-labelledby="stream-heading">
        <h2 id="stream-heading" className="text-xl font-semibold text-foreground">
          {t('stream.title')}
        </h2>

        <p className="mt-2 text-sm text-muted-foreground">
          {live.received > 0 ? t('stream.received', { count: live.received }) : t('stream.none')}
        </p>

        {live.last && (
          <p className="tabular mt-1 text-sm" data-testid="last-event">
            {t('stream.lastEvent', { entity: live.last.entity, id: live.last.entity_id })}
          </p>
        )}

        {import.meta.env.DEV && (
          <Button
            variant="secondary"
            className="mt-4"
            disabled={sendProbe.isPending}
            onClick={() => {
              sendProbe.mutate()
            }}
          >
            {sendProbe.isPending ? t('stream.sending') : t('stream.sendProbe')}
          </Button>
        )}
      </section>
    </div>
  )
}
