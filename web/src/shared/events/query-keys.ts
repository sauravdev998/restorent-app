/**
 * The query keys the live stream knows how to invalidate, and the map from an
 * event to them.
 *
 * Every key here starts with the entity kind the server names in its events.
 * That is not a naming convention, it is the mechanism: an `order_line` event
 * can invalidate exactly the queries built from order lines because those
 * queries are the ones whose key begins with `order_round` or `visit`, and the
 * map below says so out loud.
 *
 * Before this file, one event invalidated every query whose key began with the
 * entity string, which in practice meant nothing matched and the screens leaned
 * on the refetch that happens when the stream opens. Widening that to "refetch
 * everything on every event" would work and would also mean a busy kitchen
 * refetching the menu, the floor, and every open table each time one dish is
 * marked done.
 */

/** The entity kinds the server can send. Mirrors `EntityKind` in the API. */
export const ENTITY_KINDS = [
  'visit',
  'order_round',
  'order_line',
  'bill',
  'dish',
  'menu_category',
  'dining_table',
  'table_section',
  'staff',
  'probe',
] as const

/** One of them. */
export type EntityKind = (typeof ENTITY_KINDS)[number]

/** The floor: every live table and its occupancy. */
export const floorKey = ['visit', 'floor'] as const

/**
 * The admin's whole floor, live and archived, with each table's occupancy.
 *
 * Under the waiter's floor key, and so under `visit`, on purpose: every `visit`
 * event (a table opened or closed) already invalidates `['visit']`, which is
 * what keeps the admin's occupied mark current with no row of its own below.
 * An admin floor write invalidates `floorKey` on success, which reaches both.
 */
export const adminFloorKey = ['visit', 'floor', 'admin'] as const

/** One table's whole meal. */
export function visitKey(visitId: string) {
  return ['visit', visitId] as const
}

/** The kitchen queue. */
export const kitchenKey = ['order_round', 'kitchen'] as const

/** The ordering menu, which the waiter orders from and the chef's Menu tab switches. */
export const menuKey = ['dish', 'menu'] as const

/**
 * The admin's whole menu, live and archived.
 *
 * Under `dish` beside the ordering menu, so one invalidation of `['dish']`
 * refreshes both, which is what every admin menu write does on success and
 * what a `menu_category` event does for everybody else.
 */
export const adminMenuKey = ['dish', 'admin'] as const

/**
 * The admin's staff list.
 *
 * Deliberately absent from the map below, and that is a decision rather than an
 * omission. A staff change reaches everybody who needs it the only way that
 * matters: their session stops working, on their very next request. The one
 * screen that shows the list is the one the admin is looking at, and it
 * invalidates this key itself in `onSuccess`.
 *
 * The cost is that a second admin with this screen open sees a stale list until
 * they act, which is rare and harmless: their write is refused as stale, or
 * refused for a reason the screen then shows.
 */
export const staffKey = ['staff', 'list'] as const

/**
 * Which key prefixes each kind of event invalidates.
 *
 * Read it as "what does a screen have to go and ask again about". A dish going
 * unavailable changes the menu, and it also changes every open table, because a
 * waiter with the ordering screen open on one is holding that menu. A line
 * changing status changes the kitchen queue and the table it belongs to, but
 * touches neither the menu nor the floor's occupancy.
 *
 * `visit` covers the floor and every open table at once, because both are built
 * from visits and both start with that word.
 *
 * A category changing (added, renamed, reordered, archived, restored) reaches
 * the same two menus a dish does, and nothing else: no open table holds a
 * category of its own, so `visit` is left alone.
 *
 * A table changing reaches both floors and every open table under `visit`, and
 * the kitchen queue, because the table screen and every ticket show its label
 * and read it live. A section changing reaches the two floors only, which
 * `visit` covers; nothing else shows a section's name.
 *
 * `probe` invalidates nothing. It carries no product meaning: it exists so the
 * development endpoint can prove the whole path with no data behind it.
 */
export const FAN_OUT: Readonly<Record<EntityKind, readonly (readonly string[])[]>> = {
  visit: [['visit']],
  order_round: [['order_round'], ['visit']],
  order_line: [['order_round'], ['visit']],
  bill: [['visit']],
  dish: [['dish'], ['visit']],
  menu_category: [['dish']],
  dining_table: [['visit'], kitchenKey],
  table_section: [['visit']],
  staff: [floorKey],
  probe: [],
}

/** Whether this is an entity kind the map knows. */
export function isEntityKind(value: string): value is EntityKind {
  return (ENTITY_KINDS as readonly string[]).includes(value)
}
