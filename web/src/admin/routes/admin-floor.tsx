import { useQuery } from '@tanstack/react-query'
import { FolderPlus, LayoutGrid, Pencil, Plus, Trash2, Users } from 'lucide-react'
import { useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import {
  adminFloorQuery,
  archiveSection,
  archiveTable,
  isSection,
  reorderSections,
  reorderTables,
  type AdminFloor,
  type AdminTable,
  type ArchivedSection,
  type ArchivedTable,
  type FloorGroup,
  type LiveSection,
} from '@/admin/api/floor'
import { ConfirmDialog } from '@/admin/confirm-dialog'
import { ArchivedFloorSection } from '@/admin/floor/archived-floor'
import { RestoreSectionDialog, RestoreTableDialog } from '@/admin/floor/restore-dialogs'
import { SectionDialog } from '@/admin/floor/section-dialog'
import { TableDialog } from '@/admin/floor/table-dialog'
import { ReorderableList } from '@/admin/shared/reorderable'
import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { floorKey } from '@/shared/events/query-keys'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'
import { Icon } from '@/shared/ui/icon'
import { RestaurantText } from '@/shared/ui/restaurant-text'
import { Skeleton } from '@/shared/ui/skeleton'

/** Which dialog is open, and what it was opened from. */
type Opened =
  | { kind: 'table'; sectionId?: string | null; table?: AdminTable }
  | { kind: 'section'; section?: LiveSection }
  | { kind: 'removeTable'; table: AdminTable }
  | { kind: 'removeSection'; section: LiveSection }
  | { kind: 'restoreTable'; table: ArchivedTable }
  | { kind: 'restoreSection'; section: ArchivedSection }
  | null

/**
 * The admin's floor: the restaurant's sections and the tables in them, in the
 * order staff walk the room, with every change made in place. Spec 0010.
 *
 * One page, shaped like the menu screen on purpose, so there is one pattern to
 * learn. The tables with no section come first, then each section as a stacked
 * group. Adding or editing opens a dialog over it.
 *
 * **Every change is live the moment it saves.** There is no draft, and the page
 * says so, because an admin reworking the floor mid service is changing every
 * waiter's screen one save at a time.
 *
 * **The occupied mark follows the floor live.** It is read with the tables, and
 * every table opened or closed refreshes it, because this screen's key sits
 * under `visit`. It is a word and an icon, never a colour alone.
 *
 * **Nothing is written into the cache ahead of the server.** A write that
 * succeeds invalidates both floors, which refreshes this screen at once;
 * everybody else's screen learns of it from the event.
 */
export function AdminFloorScreen() {
  const { t } = useTranslation(['admin', 'common'])
  const { t: common } = useTranslation()

  const floor = useQuery(adminFloorQuery)
  const [opened, setOpened] = useState<Opened>(null)

  const heading = (
    <div className="max-w-prose">
      <h1 className="text-2xl font-semibold text-foreground">{t('floor.title')}</h1>
      <p className="mt-1 text-sm text-muted-foreground">{t('floor.intro')}</p>
    </div>
  )

  if (floor.isPending) {
    return (
      <div className="space-y-6">
        {heading}
        <Skeleton className="h-40 w-full" label={common('loading.label')} />
        <Skeleton className="h-40 w-full" />
      </div>
    )
  }

  if (floor.isError) {
    return (
      <div className="space-y-6">
        {heading}
        <EmptyState
          icon={LayoutGrid}
          title={common('error.title')}
          description={apiErrorMessage(failureBody(floor.error), common)}
          action={
            <Button
              onClick={() => {
                void floor.refetch()
              }}
            >
              {common('error.retry')}
            </Button>
          }
        />
      </div>
    )
  }

  const { groups } = floor.data
  const sections = groups.filter(isSection)
  const loose = groups.find((group) => !isSection(group))
  const tables = groups.flatMap((group) => group.tables)
  const occupied = tables.filter((table) => table.occupied).length
  const empty = tables.length === 0 && sections.length === 0

  /** The group a live table is shown in, for its edit form. */
  const groupOf = (table: AdminTable): string | null =>
    sections.find((section) => section.tables.some((each) => each.id === table.id))?.id ?? null

  const groupActions = {
    onAddTable: (sectionId: string | null) => {
      setOpened({ kind: 'table', sectionId })
    },
    onEditTable: (table: AdminTable) => {
      setOpened({ kind: 'table', table })
    },
    onRemoveTable: (table: AdminTable) => {
      setOpened({ kind: 'removeTable', table })
    },
  }

  return (
    <div className="space-y-6">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div className="space-y-2">
          {heading}
          {!empty && (
            <p className="text-sm text-muted-foreground">
              {[
                t('floor.sectionCount', { count: sections.length }),
                t('floor.tableCount', { count: tables.length }),
                ...(occupied > 0 ? [t('floor.occupiedCount', { count: occupied })] : []),
              ].join(' · ')}
            </p>
          )}
        </div>

        {!empty && (
          <div className="flex flex-wrap gap-2">
            <Button
              variant="secondary"
              onClick={() => {
                setOpened({ kind: 'section' })
              }}
            >
              <Icon icon={FolderPlus} size="sm" />
              {t('floor.addSection')}
            </Button>
            <Button
              onClick={() => {
                setOpened({ kind: 'table', sectionId: null })
              }}
            >
              <Icon icon={Plus} size="sm" />
              {t('floor.addTables')}
            </Button>
          </div>
        )}
      </header>

      {empty ? (
        <EmptyState
          icon={LayoutGrid}
          title={t('floor.emptyTitle')}
          description={t('floor.emptyBody')}
          action={
            <div className="flex flex-wrap justify-center gap-2">
              <Button
                onClick={() => {
                  setOpened({ kind: 'table', sectionId: null })
                }}
              >
                <Icon icon={Plus} size="sm" />
                {t('floor.addFirstTable')}
              </Button>
              <Button
                variant="secondary"
                onClick={() => {
                  setOpened({ kind: 'section' })
                }}
              >
                <Icon icon={FolderPlus} size="sm" />
                {t('floor.addSection')}
              </Button>
            </div>
          }
        />
      ) : (
        <>
          {loose && loose.tables.length > 0 && (
            <GroupCard group={loose} handle={null} {...groupActions} />
          )}

          {sections.length > 0 && (
            <ReorderableList
              items={sections}
              nameOf={(section) => section.name}
              kind="section"
              save={reorderSections}
              invalidateKey={floorKey}
              className="space-y-6"
            >
              {(section, handle) => (
                <GroupCard
                  group={section}
                  handle={handle}
                  {...groupActions}
                  onRename={() => {
                    setOpened({ kind: 'section', section })
                  }}
                  onRemove={() => {
                    setOpened({ kind: 'removeSection', section })
                  }}
                />
              )}
            </ReorderableList>
          )}
        </>
      )}

      <ArchivedFloorSection
        archived={floor.data.archived}
        onRestoreSection={(section) => {
          setOpened({ kind: 'restoreSection', section })
        }}
        onRestoreTable={(table) => {
          setOpened({ kind: 'restoreTable', table })
        }}
      />

      <Dialogs
        opened={opened}
        floor={floor.data}
        groupOf={groupOf}
        onClose={() => {
          setOpened(null)
        }}
      />
    </div>
  )
}

interface DialogsProps {
  opened: Opened
  floor: AdminFloor
  groupOf: (table: AdminTable) => string | null
  onClose: () => void
}

/** Whichever dialog is open. */
function Dialogs({ opened, floor, groupOf, onClose }: DialogsProps) {
  const { t } = useTranslation('admin')

  if (opened === null) return null

  const onOpenChange = (open: boolean) => {
    if (!open) onClose()
  }

  switch (opened.kind) {
    case 'table':
      return (
        <TableDialog
          onOpenChange={onOpenChange}
          floor={floor}
          {...(opened.sectionId === undefined ? {} : { sectionId: opened.sectionId })}
          {...(opened.table === undefined
            ? {}
            : { table: { table: opened.table, sectionId: groupOf(opened.table) } })}
        />
      )

    case 'section':
      return (
        <SectionDialog
          onOpenChange={onOpenChange}
          {...(opened.section === undefined ? {} : { section: opened.section })}
        />
      )

    case 'removeTable':
      return (
        <ConfirmDialog
          onOpenChange={onOpenChange}
          title={t('floor.remove.tableTitle', { table: opened.table.label })}
          description={
            opened.table.occupied ? t('floor.remove.tableBusy') : t('floor.remove.tableBody')
          }
          confirmLabel={t('floor.remove.tableConfirm')}
          doneMessage={t('floor.remove.tableDone', { table: opened.table.label })}
          action={() => archiveTable(opened.table.id)}
          invalidateKey={floorKey}
        />
      )

    case 'removeSection':
      return (
        <ConfirmDialog
          onOpenChange={onOpenChange}
          title={t('floor.remove.sectionTitle', { section: opened.section.name })}
          description={
            opened.section.tables.length > 0
              ? t('floor.remove.sectionNotEmpty', { count: opened.section.tables.length })
              : t('floor.remove.sectionBody')
          }
          confirmLabel={t('floor.remove.sectionConfirm')}
          doneMessage={t('floor.remove.sectionDone', { section: opened.section.name })}
          action={() => archiveSection(opened.section.id)}
          invalidateKey={floorKey}
        />
      )

    case 'restoreTable':
      return <RestoreTableDialog onOpenChange={onOpenChange} table={opened.table} floor={floor} />

    case 'restoreSection':
      return <RestoreSectionDialog onOpenChange={onOpenChange} section={opened.section} />
  }
}

interface GroupCardProps {
  /** A live section, or the group of tables with no section. */
  group: FloorGroup
  /** The drag handle that moves the whole section, or `null` for no section. */
  handle: ReactNode
  onAddTable: (sectionId: string | null) => void
  onEditTable: (table: AdminTable) => void
  onRemoveTable: (table: AdminTable) => void
  onRename?: () => void
  onRemove?: () => void
}

/** One group: its heading, its actions, and its tables in walking order. */
function GroupCard({
  group,
  handle,
  onAddTable,
  onEditTable,
  onRemoveTable,
  onRename,
  onRemove,
}: GroupCardProps) {
  const { t } = useTranslation('admin')
  const section = isSection(group) ? group : null
  const headingId = `floor-group-${section?.id ?? 'none'}`
  const title = section?.name ?? t('floor.unsectioned')

  return (
    <Card as="section" aria-labelledby={headingId} className="space-y-3">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          {handle}
          <h2 id={headingId} className="truncate text-lg font-semibold text-card-foreground">
            {section ? <RestaurantText>{section.name}</RestaurantText> : title}
          </h2>
          <span className="text-sm text-muted-foreground">
            {t('floor.tableCount', { count: group.tables.length })}
          </span>
        </div>

        <div className="flex flex-wrap gap-2">
          {section && onRename && (
            <Button
              variant="ghost"
              size="sm"
              aria-label={t('floor.renameSectionNamed', { section: section.name })}
              onClick={onRename}
            >
              <Icon icon={Pencil} size="sm" />
              {t('floor.renameSection')}
            </Button>
          )}
          {section && onRemove && (
            <Button
              variant="ghost"
              size="sm"
              aria-label={t('floor.remove.sectionNamed', { section: section.name })}
              onClick={onRemove}
            >
              <Icon icon={Trash2} size="sm" />
              {t('floor.remove.action')}
            </Button>
          )}
          <Button
            variant="secondary"
            size="sm"
            aria-label={
              section
                ? t('floor.addTableTo', { section: section.name })
                : t('floor.addTableUnsectioned')
            }
            onClick={() => {
              onAddTable(section?.id ?? null)
            }}
          >
            <Icon icon={Plus} size="sm" />
            {t('floor.addTableHere')}
          </Button>
        </div>
      </header>

      {group.tables.length === 0 ? (
        <p className="border-line rounded-md border-dashed border-border p-4 text-sm text-muted-foreground">
          {t('floor.sectionEmpty')}
        </p>
      ) : (
        <ReorderableList
          items={group.tables}
          nameOf={(table) => table.label}
          kind="table"
          save={(ids) => reorderTables(section?.id ?? null, ids)}
          invalidateKey={floorKey}
          className="divide-y divide-border"
          itemClassName="bg-card"
        >
          {(table, tableHandle) => (
            <TableRow
              table={table}
              handle={tableHandle}
              onEdit={() => {
                onEditTable(table)
              }}
              onRemove={() => {
                onRemoveTable(table)
              }}
            />
          )}
        </ReorderableList>
      )}
    </Card>
  )
}

interface TableRowProps {
  table: AdminTable
  /** The drag handle that moves this table within its group. */
  handle: ReactNode
  onEdit: () => void
  onRemove: () => void
}

/** One table: its label, its seats, and whether a party is at it. */
function TableRow({ table, handle, onEdit, onRemove }: TableRowProps) {
  const { t } = useTranslation('admin')

  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2 py-3">
      {handle}

      <div className="min-w-0 flex-1">
        <p className="truncate text-base font-medium text-card-foreground">
          <RestaurantText>{table.label}</RestaurantText>
        </p>
        {table.seats !== null && (
          <p className="text-sm text-muted-foreground">
            {t('floor.seats', { count: table.seats })}
          </p>
        )}
      </div>

      {table.occupied ? (
        <span className="inline-flex min-w-24 items-center gap-1.5 text-sm font-medium text-card-foreground">
          <Icon icon={Users} size="sm" />
          {t('floor.occupied')}
        </span>
      ) : (
        <span className="min-w-24 text-sm text-muted-foreground">{t('floor.free')}</span>
      )}

      <Button
        variant="ghost"
        size="sm"
        aria-label={t('floor.editTableNamed', { table: table.label })}
        onClick={onEdit}
      >
        <Icon icon={Pencil} size="sm" />
        {t('floor.editTable')}
      </Button>
      <Button
        variant="ghost"
        size="sm"
        aria-label={t('floor.remove.tableNamed', { table: table.label })}
        onClick={onRemove}
      >
        <Icon icon={Trash2} size="sm" />
        {t('floor.remove.action')}
      </Button>
    </div>
  )
}
