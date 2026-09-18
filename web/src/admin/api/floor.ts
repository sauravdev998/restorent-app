import { queryOptions } from '@tanstack/react-query'

import { ApiCallError } from '@/shared/api/call-error'
import { api } from '@/shared/api/client'
import type { components } from '@/shared/api/schema'
import { adminFloorKey } from '@/shared/events/query-keys'

/**
 * Everything the admin's floor screen asks the API for, spec 0010.
 *
 * One read and a set of writes. Every write answers with what it wrote, but
 * the screen does not write that into the cache: it invalidates the floor key
 * on success and reads the floor again. That keeps spec 0007's rule of no
 * cache optimism, and it is what makes the admin's own screen change at once
 * rather than a second later when the event arrives.
 */

/** The whole floor, live and archived. */
export type AdminFloor = components['schemas']['AdminFloorResponse']

/** One live group: a section, or the tables with no section. */
export type FloorGroup = components['schemas']['AdminFloorGroupDto']

/** One live table, with whether a party is at it. */
export type AdminTable = components['schemas']['AdminTableDto']

/** Everything in the Archived section. */
export type ArchivedFloor = AdminFloor['archived']

/** One archived section, with the tables that can come back with it. */
export type ArchivedSection = ArchivedFloor['sections'][number]

/** One archived table, with what the restore dialog needs. */
export type ArchivedTable = ArchivedFloor['tables'][number]

/** A section as every write on one answers with it. */
export type Section = components['schemas']['SectionDto']

/** A table as every write on one answers with it. */
export type Table = components['schemas']['TableDto']

/** A live section, which is a group that has an id, a name, and a version. */
export interface LiveSection extends FloorGroup {
  id: string
  name: string
  version: number
}

/** Whether a group is a live section rather than the no section group. */
export function isSection(group: FloorGroup): group is LiveSection {
  return typeof group.id === 'string' && typeof group.name === 'string'
}

/** The admin's whole floor, for the floor screen. */
export const adminFloorQuery = queryOptions({
  queryKey: adminFloorKey,
  queryFn: async (): Promise<AdminFloor> => {
    const { data, error } = await api.GET('/api/admin/floor')
    if (!data) throw new ApiCallError(error)
    return data
  },
})

/** Adds a section at the end of the list. */
export async function createSection(name: string): Promise<Section> {
  const { data, error } = await api.POST('/api/admin/floor/sections', { body: { name } })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Renames a section, naming the version the form loaded. */
export async function renameSection(
  sectionId: string,
  name: string,
  version: number,
): Promise<Section> {
  const { data, error } = await api.PUT('/api/admin/floor/sections/{id}', {
    params: { path: { id: sectionId } },
    body: { name, version },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Puts the sections in a new order: the complete list, in that order. */
export async function reorderSections(ids: string[]): Promise<Section[]> {
  const { data, error } = await api.PUT('/api/admin/floor/sections/order', { body: { ids } })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Takes a section off the floor. Refused while it still holds a live table. */
export async function archiveSection(sectionId: string): Promise<Section> {
  const { data, error } = await api.POST('/api/admin/floor/sections/{id}/archive', {
    params: { path: { id: sectionId } },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Puts an archived section back, with the chosen tables that were in it. */
export async function restoreSection(
  sectionId: string,
  tableIds: string[],
): Promise<components['schemas']['RestoredSectionDto']> {
  const { data, error } = await api.POST('/api/admin/floor/sections/{id}/restore', {
    params: { path: { id: sectionId } },
    body: { tableIds },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** What the one table form collects, already turned into what the API takes. */
export interface TableForm {
  /** The section, or `null` for none. */
  sectionId: string | null
  label: string
  seats: number | null
}

/** Adds one table at the end of its group. */
export async function createTable(form: TableForm): Promise<Table> {
  const { data, error } = await api.POST('/api/admin/floor/tables', {
    body: { sectionId: form.sectionId, label: form.label, seats: form.seats },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** What the range form collects, already turned into what the API takes. */
export interface TableRangeForm {
  sectionId: string | null
  prefix: string
  from: number
  to: number
  seats: number | null
}

/** Adds a numbered range of tables at the end of their group, all or none. */
export async function createTableRange(form: TableRangeForm): Promise<Table[]> {
  const { data, error } = await api.POST('/api/admin/floor/tables/range', {
    body: {
      sectionId: form.sectionId,
      prefix: form.prefix,
      from: form.from,
      to: form.to,
      seats: form.seats,
    },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/**
 * Edits a table, naming the version the form loaded.
 *
 * A different section moves it to the end of that group. Allowed while a party
 * sits at it.
 */
export async function editTable(tableId: string, form: TableForm, version: number): Promise<Table> {
  const { data, error } = await api.PUT('/api/admin/floor/tables/{id}', {
    params: { path: { id: tableId } },
    body: { sectionId: form.sectionId, label: form.label, seats: form.seats, version },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/**
 * Puts one group's tables in a new order: the complete list, in that order.
 *
 * `null` is the group of tables with no section.
 */
export async function reorderTables(sectionId: string | null, ids: string[]): Promise<Table[]> {
  const { data, error } = await api.PUT('/api/admin/floor/table-order', {
    body: { sectionId, ids },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Takes a table off the floor. Refused while a party is at it. */
export async function archiveTable(tableId: string): Promise<Table> {
  const { data, error } = await api.POST('/api/admin/floor/tables/{id}/archive', {
    params: { path: { id: tableId } },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Puts an archived table back at the end of a live section, or of no section. */
export async function restoreTable(tableId: string, sectionId: string | null): Promise<Table> {
  const { data, error } = await api.POST('/api/admin/floor/tables/{id}/restore', {
    params: { path: { id: tableId } },
    body: { sectionId },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/**
 * The labels a `409 labels_taken` refusal names, or `null` for any other
 * failure.
 *
 * An empty list is a real answer: a label was taken and freed again between
 * the check and the write, and the form says to try again.
 */
export function clashingLabels(body: unknown): string[] | null {
  if (typeof body !== 'object' || body === null) return null

  const { error, labels } = body as { error?: unknown; labels?: unknown }
  if (error !== 'labels_taken' || !Array.isArray(labels)) return null

  return labels.filter((label): label is string => typeof label === 'string')
}
