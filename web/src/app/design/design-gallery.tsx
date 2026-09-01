import { Inbox } from 'lucide-react'
import { useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { Alert } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { cn } from '@/shared/ui/cn'
import { DataTable, type DataTableColumn, type SortDirection } from '@/shared/ui/data-table'
import { Dialog } from '@/shared/ui/dialog'
import { ElapsedTime } from '@/shared/ui/elapsed-time'
import { EmptyState } from '@/shared/ui/empty-state'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { Skeleton } from '@/shared/ui/skeleton'
import { STATUS_TONES } from '@/shared/ui/status'
import { StatusPill } from '@/shared/ui/status-pill'
import { showToast } from '@/shared/ui/toast-store'

/** Every semantic role, so a missing or wrong one is visible rather than theoretical. */
const SURFACE_ROLES = ['background', 'card', 'muted', 'secondary', 'popover'] as const
const INK_ROLES = ['foreground', 'muted-foreground', 'primary', 'destructive'] as const
const LINE_ROLES = ['border', 'input', 'ring'] as const

const TYPE_STEPS = ['text-xs', 'text-sm', 'text-base', 'text-lg', 'text-xl', 'text-2xl'] as const

const DENSITIES = ['admin', 'waiter', 'kitchen'] as const
type Density = (typeof DENSITIES)[number]

const APPEARANCES = ['dark', 'light'] as const

interface SampleRow {
  id: string
  table: string
  status: (typeof STATUS_TONES)[number]
  since: string
}

const SAMPLE_ROWS: SampleRow[] = [
  { id: '1', table: '4', status: 'queued', since: new Date(Date.now() - 65_000).toISOString() },
  { id: '2', table: '9', status: 'ready', since: new Date(Date.now() - 240_000).toISOString() },
  { id: '3', table: '2', status: 'served', since: new Date(Date.now() - 1_500_000).toISOString() },
]

function Swatch({ role, kind }: { role: string; kind: 'surface' | 'ink' | 'line' }) {
  return (
    <div className="flex items-center gap-3">
      <span
        className={cn(
          'border-line size-8 shrink-0 rounded-md border-border',
          kind === 'line' && 'border-4',
        )}
        style={
          kind === 'ink'
            ? { backgroundColor: `var(--${role})` }
            : kind === 'line'
              ? { borderColor: `var(--${role})`, backgroundColor: 'var(--card)' }
              : { backgroundColor: `var(--${role})` }
        }
      />
      <code className="text-xs text-muted-foreground">--{role}</code>
    </div>
  )
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-3">
      <h3 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
        {title}
      </h3>
      {children}
    </section>
  )
}

/**
 * Everything the design system contains, in one panel, at one appearance and
 * one density.
 *
 * Rendered twice side by side, so a colour that works in dark and fails in
 * light is visible rather than discovered by a restaurant.
 */
function Panel({ appearance, density }: { appearance: string; density: Density }) {
  const { t } = useTranslation()
  const [sort, setSort] = useState<{ key: string; direction: SortDirection }>({
    key: 'table',
    direction: 'ascending',
  })

  const columns: DataTableColumn<SampleRow>[] = [
    { key: 'table', header: 'Table', cell: (row) => row.table, sortable: true, rowHeader: true },
    { key: 'status', header: 'Status', cell: (row) => <StatusPill status={row.status} /> },
    { key: 'since', header: 'Waiting', cell: (row) => <ElapsedTime since={row.since} /> },
  ]

  return (
    <div
      data-theme={appearance}
      data-surface={density}
      className="border-line flex flex-col gap-8 rounded-lg border-border bg-background p-6 text-foreground"
    >
      <Section title={t('design.palette')}>
        <div className="grid gap-3 sm:grid-cols-2">
          {SURFACE_ROLES.map((role) => (
            <Swatch key={role} role={role} kind="surface" />
          ))}
          {INK_ROLES.map((role) => (
            <Swatch key={role} role={role} kind="ink" />
          ))}
          {LINE_ROLES.map((role) => (
            <Swatch key={role} role={role} kind="line" />
          ))}
        </div>
      </Section>

      <Section title={t('design.statuses')}>
        <div className="flex flex-wrap gap-3">
          {STATUS_TONES.map((status) => (
            <StatusPill key={status} status={status} />
          ))}
        </div>
      </Section>

      <Section title={t('design.type')}>
        <div className="flex flex-col gap-1">
          {TYPE_STEPS.map((step) => (
            <p key={step} className={step}>
              {step}
            </p>
          ))}
          <p className="tabular text-sm">1,204.50 · 12:07 · #000418</p>
        </div>
      </Section>

      <Section title={t('design.buttons')}>
        <div className="flex flex-wrap items-center gap-3">
          <Button>{t('design.confirm')}</Button>
          <Button variant="secondary">{t('design.cancel')}</Button>
          <Button variant="ghost">{t('design.cancel')}</Button>
          <Button variant="destructive">{t('status.voided')}</Button>
          <Button disabled>{t('design.confirm')}</Button>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <Button size="sm">sm</Button>
          <Button size="md">md</Button>
          <Button size="lg">lg</Button>
        </div>
      </Section>

      <Section title={t('design.cards')}>
        <Card as="article" className="flex flex-col gap-3">
          <div className="flex items-center justify-between gap-4">
            <p className="text-base font-semibold">Table 4 · round 2</p>
            <StatusPill status="queued" />
          </div>
          <ElapsedTime since={SAMPLE_ROWS[0]?.since ?? new Date().toISOString()} />
        </Card>
      </Section>

      <Section title={t('design.forms')}>
        <Field label={t('design.sampleField')} hint={t('design.sampleHint')} required>
          <Input placeholder="Grilled sea bass" />
        </Field>
        <Field label={t('design.sampleField')} error={t('design.sampleError')}>
          <Input defaultValue="" />
        </Field>
        <Field label={t('design.sampleSelect')}>
          <Select defaultValue="main">
            <option value="starter">{t('course.starter')}</option>
            <option value="main">{t('course.main')}</option>
            <option value="dessert">{t('course.dessert')}</option>
          </Select>
        </Field>
      </Section>

      <Section title={t('design.feedback')}>
        <Alert
          open
          title={t('design.alertTitle')}
          description={t('design.alertBody')}
          sound={false}
        />
        <div className="flex flex-col gap-2">
          <Skeleton label={t('loading.label')} className="w-2/3" />
          <Skeleton className="w-full" />
          <Skeleton className="w-1/2" />
        </div>
        <EmptyState
          icon={Inbox}
          title={t('design.emptyTitle')}
          description={t('design.emptyBody')}
          action={<Button variant="secondary">{t('design.emptyAction')}</Button>}
        />
      </Section>

      <Section title={t('design.data')}>
        <DataTable
          caption={t('design.data')}
          captionHidden
          columns={columns}
          rows={SAMPLE_ROWS}
          rowKey={(row) => row.id}
          sort={sort}
          onSortChange={(key) => {
            setSort((current) => ({
              key,
              direction:
                current.key === key && current.direction === 'ascending'
                  ? 'descending'
                  : 'ascending',
            }))
          }}
        />
      </Section>
    </div>
  )
}

/**
 * The design system, looking at itself.
 *
 * Mounted only when `import.meta.env.DEV` is true, so the whole module and
 * everything it imports is dropped from a production build rather than sitting
 * behind a route somebody could find.
 *
 * The two panels force their appearance with `data-theme` and their density
 * with `data-surface`, which is the entire reason those attribute hooks exist:
 * a media query is document wide, so without them the two appearances could
 * never be seen at once and the light theme would ship on trust.
 *
 * The overlays sit outside the panels on purpose. A dialog and a toast render
 * into a portal on `document.body`, so they follow the document's density
 * rather than a panel's, and pretending otherwise here would be a lie about how
 * they behave in the product.
 */
export function DesignGallery() {
  const { t } = useTranslation()
  const [density, setDensity] = useState<Density>('admin')
  const [dialogOpen, setDialogOpen] = useState(false)
  const [alertOpen, setAlertOpen] = useState(false)

  return (
    <div className="flex flex-col gap-8">
      <header className="flex flex-col gap-3">
        <h1 className="text-2xl font-semibold text-foreground">{t('design.title')}</h1>
        <p className="max-w-prose text-sm text-muted-foreground">{t('design.intro')}</p>
      </header>

      <fieldset className="flex flex-wrap items-center gap-3">
        <legend className="sr-only">{t('design.density')}</legend>
        <span className="text-sm text-muted-foreground">{t('design.density')}</span>
        {DENSITIES.map((option) => (
          <Button
            key={option}
            variant={density === option ? 'primary' : 'secondary'}
            size="sm"
            aria-pressed={density === option}
            onClick={() => {
              setDensity(option)
            }}
          >
            {t(`nav.${option}`)}
          </Button>
        ))}
      </fieldset>

      <div className="grid gap-6 lg:grid-cols-2">
        {APPEARANCES.map((appearance) => (
          <div key={appearance} className="flex flex-col gap-3">
            <h2 className="text-sm font-semibold text-foreground">
              {t(`design.${appearance}`)} · {t(`nav.${density}`)}
            </h2>
            <Panel appearance={appearance} density={density} />
          </div>
        ))}
      </div>

      <section className="flex flex-col gap-4">
        <h2 className="text-lg font-semibold text-foreground">{t('design.feedback')}</h2>
        <p className="max-w-prose text-sm text-muted-foreground">
          These render into a portal on the document, so they follow the document&rsquo;s own
          density rather than a panel&rsquo;s.
        </p>
        <div className="flex flex-wrap gap-3">
          <Button
            onClick={() => {
              setDialogOpen(true)
            }}
          >
            {t('design.openDialog')}
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              showToast({ title: t('design.toastBody'), tone: 'ready' })
            }}
          >
            {t('design.showToast')}
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              setAlertOpen(true)
            }}
          >
            {t('design.raiseAlert')}
          </Button>
        </div>

        <Alert
          open={alertOpen}
          title={t('design.alertTitle')}
          description={t('design.alertBody')}
          onDismiss={() => {
            setAlertOpen(false)
          }}
        />
      </section>

      <Dialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        title={t('design.dialogTitle')}
        description={t('design.dialogBody')}
        footer={
          <>
            <Button
              onClick={() => {
                setDialogOpen(false)
              }}
            >
              {t('design.confirm')}
            </Button>
            <Button
              variant="secondary"
              onClick={() => {
                setDialogOpen(false)
              }}
            >
              {t('design.cancel')}
            </Button>
          </>
        }
      />
    </div>
  )
}
