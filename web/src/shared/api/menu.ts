import { queryOptions } from '@tanstack/react-query'

import { api } from '@/shared/api/client'
import type { components } from '@/shared/api/schema'
import { menuKey } from '@/shared/events/query-keys'

import { ApiCallError } from './call-error'

/**
 * The ordering menu, and the one switch on it, which two surfaces share.
 *
 * The waiter orders from this menu and the chef's Menu tab switches dishes on
 * it, so both read the one cache entry. A dish switched off from the kitchen
 * invalidates it through the `dish` event, and every screen holding it
 * refetches.
 */

/** The live menu, grouped the way it is printed. */
export type Menu = components['schemas']['MenuResponse']

/** One category on it. */
export type MenuCategory = components['schemas']['MenuCategoryDto']

/** One item on the menu. */
export type MenuDish = components['schemas']['MenuDishDto']

/** Veg, non veg, or egg. */
export type Diet = components['schemas']['DietDto']

/** The three markers, in the order a menu lists them. */
export const DIETS: readonly Diet[] = ['veg', 'non_veg', 'egg']

/** A dish as every menu write answers with it. */
export type Dish = components['schemas']['DishDto']

/**
 * The menu, for the waiter's ordering screen and the chef's Menu tab.
 *
 * Kept a while, because a menu changes far less often than a floor does and a
 * waiter opens it at every table. A change arrives as a live event and
 * invalidates it, so the staleness never outlasts a change.
 */
export const menuQuery = queryOptions({
  queryKey: menuKey,
  queryFn: async (): Promise<Menu> => {
    const { data, error } = await api.GET('/api/menu')
    if (!data) throw new ApiCallError(error)
    return data
  },
  staleTime: 5 * 60_000,
})

/**
 * Switches a dish on or off.
 *
 * An absolute value, never a toggle, so a second tap made before the first
 * answer arrived still lands where the chef meant. Admins and chefs only; the
 * server refuses anybody else.
 */
export async function setDishAvailability(dishId: string, available: boolean): Promise<Dish> {
  const { data, error } = await api.PUT('/api/dishes/{id}/availability', {
    params: { path: { id: dishId } },
    body: { available },
  })

  if (!data) throw new ApiCallError(error)
  return data
}
