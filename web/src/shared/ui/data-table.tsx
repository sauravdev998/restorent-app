import { ArrowDown, ArrowUp, ArrowUpDown } from 'lucide-react'
import { useId, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { cn } from './cn'
import { Icon } from './icon'

export type SortDirection = 'ascending' | 'descending'

export interface DataTableColumn<Row> {
  /** Matches the `sort.key` this column sorts by. */
  key: string
  header: string
  cell: (row: Row) => ReactNode
  sortable?: boolean
  /**
   * Renders this column as the row's own header cell. At most one per table,
   * and it is what a screen reader repeats to say which row it is reading.
   */
  rowHeader?: boolean
}

export interface DataTableProps<Row> {
  /** What the table is a table of. Always present, visible or not. */
  caption: string
  /** Hides the caption from sight while leaving it for a screen reader. */
  captionHidden?: boolean
  columns: readonly DataTableColumn<Row>[]
  rows: readonly Row[]
  rowKey: (row: Row) => string
  sort?: { key: string; direction: SortDirection }
  onSortChange?: (key: string) => void
  /** Shown in place of the body when there are no rows. */
  empty?: ReactNode
  className?: string
}

const DIRECTION_ICON = { ascending: ArrowUp, descending: ArrowDown }

/**
 * The admin's list, as a real table.
 *
 * A real `<table>` with a caption, column headers, and one row header per row,
 * because that is what lets a screen reader say "table 4, waiter Ana, 12
 * minutes" instead of reading three numbers with no idea which row they belong
 * to. A grid of divs cannot do that, however much it looks the same.
 *
 * A sortable header is a real `<button>` inside the `<th>`, and the `<th>`
 * carries `aria-sort`, so the current sort is announced rather than only drawn.
 */
export function DataTable<Row>({
  caption,
  captionHidden = false,
  columns,
  rows,
  rowKey,
  sort,
  onSortChange,
  empty,
  className,
}: DataTableProps<Row>) {
  const { t } = useTranslation()
  const captionId = useId()

  return (
    /* The scroll container is focusable and named, which matters most on the
       waiter surface: a three column table does not fit a phone, so this box
       scrolls sideways, and a box that scrolls but cannot be focused is one a
       keyboard or switch user can never reach the end of. Naming it by the
       caption keeps the announcement honest ("Data, region") rather than
       landing on an anonymous focus stop. */
    <div
      role="region"
      aria-labelledby={captionId}
      tabIndex={0}
      className={cn('w-full overflow-x-auto', className)}
    >
      <table className="w-full border-collapse text-sm">
        <caption
          id={captionId}
          className={cn(
            'text-start',
            captionHidden ? 'sr-only' : 'pb-3 text-base font-semibold text-foreground',
          )}
        >
          {caption}
        </caption>

        <thead>
          <tr>
            {columns.map((column) => {
              const active = sort?.key === column.key
              const direction = active ? sort.direction : undefined
              const SortIcon = direction ? DIRECTION_ICON[direction] : ArrowUpDown

              return (
                <th
                  key={column.key}
                  scope="col"
                  aria-sort={direction ?? (column.sortable === true ? 'none' : undefined)}
                  className="border-b-line border-border p-3 text-start font-medium text-muted-foreground"
                >
                  {column.sortable === true && onSortChange ? (
                    <button
                      type="button"
                      className="target-h inline-flex items-center gap-2 rounded-md text-start"
                      onClick={() => {
                        onSortChange(column.key)
                      }}
                    >
                      {column.header}
                      <Icon icon={SortIcon} size="sm" />
                      {direction && (
                        <span className="sr-only">
                          {t(
                            `table.sorted${direction === 'ascending' ? 'Ascending' : 'Descending'}`,
                          )}
                        </span>
                      )}
                    </button>
                  ) : (
                    column.header
                  )}
                </th>
              )
            })}
          </tr>
        </thead>

        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td colSpan={columns.length} className="p-6 text-center text-muted-foreground">
                {empty ?? t('table.empty')}
              </td>
            </tr>
          ) : (
            rows.map((row) => (
              <tr key={rowKey(row)} className="border-b-line border-border">
                {columns.map((column) =>
                  column.rowHeader === true ? (
                    <th
                      key={column.key}
                      scope="row"
                      className="p-3 text-start font-medium text-foreground"
                    >
                      {column.cell(row)}
                    </th>
                  ) : (
                    <td key={column.key} className="p-3 text-start">
                      {column.cell(row)}
                    </td>
                  ),
                )}
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  )
}
