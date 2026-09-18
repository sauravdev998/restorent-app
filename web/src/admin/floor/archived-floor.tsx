import { ArchiveRestore } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import type { ArchivedFloor, ArchivedSection, ArchivedTable } from '@/admin/api/floor'
import { formatTimestamp } from '@/shared/format'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Icon } from '@/shared/ui/icon'
import { RestaurantText } from '@/shared/ui/restaurant-text'

export interface ArchivedFloorProps {
  archived: ArchivedFloor
  /** Opens the restore dialog for a section, which lists its tables. */
  onRestoreSection: (section: ArchivedSection) => void
  /** Opens the restore dialog for a table, which asks which section it goes into. */
  onRestoreTable: (table: ArchivedTable) => void
}

/**
 * Everything taken off the floor, and the way back for each.
 *
 * A removed table or section is off every waiter's floor, but not gone: every
 * past visit and bill still shows its label, and it comes back from here as it
 * was. A section comes back with the tables that were in it, which is what makes
 * closing a terrace for the winter cost nothing in the spring.
 */
export function ArchivedFloorSection({
  archived,
  onRestoreSection,
  onRestoreTable,
}: ArchivedFloorProps) {
  const { t } = useTranslation('admin')

  const { sections, tables } = archived

  /** Which section a removed table came from, and whether that one is gone too. */
  const whereFrom = (table: ArchivedTable): string => {
    const section = table.sectionName ?? null
    if (section === null) return t('floor.archived.noSection')
    return table.sectionLive
      ? t('floor.archived.from', { section })
      : t('floor.archived.fromRemoved', { section })
  }
  const empty = sections.length === 0 && tables.length === 0

  // Most recently removed first, which is the one an admin is most likely to
  // want back. The API sends tables in their old order, for the section dialog.
  const recentTables = [...tables].sort((a, b) => b.archivedAt.localeCompare(a.archivedAt))

  return (
    <section aria-labelledby="archived-floor-heading" className="space-y-3">
      <div className="max-w-prose">
        <h2 id="archived-floor-heading" className="text-lg font-semibold text-foreground">
          {t('floor.archived.title')}
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">{t('floor.archived.intro')}</p>
      </div>

      {empty ? (
        <p className="border-line rounded-md border-dashed border-border p-4 text-sm text-muted-foreground">
          {t('floor.archived.empty')}
        </p>
      ) : (
        <Card className="space-y-4">
          {sections.length > 0 && (
            <div className="space-y-1">
              <h3 className="text-sm font-medium text-muted-foreground">
                {t('floor.archived.sections')}
              </h3>
              <ul className="divide-y divide-border">
                {sections.map((section) => (
                  <li key={section.id} className="flex flex-wrap items-center gap-x-4 gap-y-1 py-2">
                    <div className="min-w-0 flex-1">
                      <RestaurantText as="p" className="truncate font-medium text-card-foreground">
                        {section.name}
                      </RestaurantText>
                      {section.tables.length > 0 && (
                        <p className="truncate text-xs text-muted-foreground">
                          {t('floor.archived.withTables', { count: section.tables.length })}
                        </p>
                      )}
                    </div>
                    <span className="text-sm text-muted-foreground">
                      {t('floor.archived.removedAt', { when: formatTimestamp(section.archivedAt) })}
                    </span>
                    <Button
                      variant="secondary"
                      size="sm"
                      aria-label={t('floor.archived.restoreSectionNamed', { name: section.name })}
                      onClick={() => {
                        onRestoreSection(section)
                      }}
                    >
                      <Icon icon={ArchiveRestore} size="sm" />
                      {t('floor.archived.restore')}
                    </Button>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {tables.length > 0 && (
            <div className="space-y-1">
              <h3 className="text-sm font-medium text-muted-foreground">
                {t('floor.archived.tables')}
              </h3>
              <ul className="divide-y divide-border">
                {recentTables.map((table) => (
                  <li key={table.id} className="flex flex-wrap items-center gap-x-4 gap-y-1 py-2">
                    <div className="min-w-0 flex-1">
                      <RestaurantText as="p" className="truncate font-medium text-card-foreground">
                        {table.label}
                      </RestaurantText>
                      <p className="truncate text-xs text-muted-foreground">{whereFrom(table)}</p>
                    </div>
                    <span className="text-sm text-muted-foreground">
                      {t('floor.archived.removedAt', { when: formatTimestamp(table.archivedAt) })}
                    </span>
                    <Button
                      variant="secondary"
                      size="sm"
                      aria-label={t('floor.archived.restoreTableNamed', { name: table.label })}
                      onClick={() => {
                        onRestoreTable(table)
                      }}
                    >
                      <Icon icon={ArchiveRestore} size="sm" />
                      {t('floor.archived.restore')}
                    </Button>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </Card>
      )}
    </section>
  )
}
