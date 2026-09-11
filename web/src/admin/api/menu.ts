import { queryOptions } from '@tanstack/react-query'

import { ApiCallError } from '@/shared/api/call-error'
import { api } from '@/shared/api/client'
import type { Diet, Dish } from '@/shared/api/menu'
import type { components } from '@/shared/api/schema'
import { adminMenuKey } from '@/shared/events/query-keys'

/**
 * Everything the admin's menu screen asks the API for.
 *
 * One read and a handful of writes. Every write answers with the row it wrote,
 * but the screen does not write that row into the cache: it invalidates
 * `['dish']` on success and reads the menu again. That is spec 0007's rule of
 * no cache optimism kept, and it is also what makes the admin's own screen
 * change at once instead of waiting on the event round trip.
 */

/** The whole menu, live and archived. */
export type AdminMenu = components['schemas']['AdminMenuResponse']

/** One live category with its dishes. */
export type AdminCategory = components['schemas']['AdminCategoryDto']

/** The admin's whole menu, for the menu screen. */
export const adminMenuQuery = queryOptions({
  queryKey: adminMenuKey,
  queryFn: async (): Promise<AdminMenu> => {
    const { data, error } = await api.GET('/api/admin/menu')
    if (!data) throw new ApiCallError(error)
    return data
  },
})

/** What the dish form collects, exactly as typed. */
export interface DishForm {
  categoryId: string
  name: string
  description: string
  /** The price box, already turned into a plain decimal where it could be. */
  price: string
  diet: Diet
}

/** Adds a dish to the end of its category, available. */
export async function createDish(form: DishForm): Promise<Dish> {
  const { data, error } = await api.POST('/api/admin/menu/dishes', {
    body: {
      categoryId: form.categoryId,
      name: form.name,
      description: form.description,
      price: form.price,
      diet: form.diet,
    },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** A category as every write on one answers with it. */
export type Category = components['schemas']['CategoryDto']

/** Adds a category to the end of the category list. */
export async function createCategory(name: string): Promise<Category> {
  const { data, error } = await api.POST('/api/admin/menu/categories', { body: { name } })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Renames a category, naming the version the form loaded. */
export async function renameCategory(
  categoryId: string,
  name: string,
  version: number,
): Promise<Category> {
  const { data, error } = await api.PUT('/api/admin/menu/categories/{id}', {
    params: { path: { id: categoryId } },
    body: { name, version },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/**
 * Edits a dish, naming the version the form loaded.
 *
 * A different category moves it to the end of that category. Availability is
 * not in here and cannot be: only the switch writes it.
 */
export async function editDish(dishId: string, form: DishForm, version: number): Promise<Dish> {
  const { data, error } = await api.PUT('/api/admin/menu/dishes/{id}', {
    params: { path: { id: dishId } },
    body: {
      categoryId: form.categoryId,
      name: form.name,
      description: form.description,
      price: form.price,
      diet: form.diet,
      version,
    },
  })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Puts the categories in a new order: the complete list, in that order. */
export async function reorderCategories(ids: string[]): Promise<Category[]> {
  const { data, error } = await api.PUT('/api/admin/menu/categories/order', { body: { ids } })

  if (!data) throw new ApiCallError(error)
  return data
}

/** Puts one category's dishes in a new order: the complete list, in that order. */
export async function reorderDishes(categoryId: string, ids: string[]): Promise<Dish[]> {
  const { data, error } = await api.PUT('/api/admin/menu/categories/{id}/dish-order', {
    params: { path: { id: categoryId } },
    body: { ids },
  })

  if (!data) throw new ApiCallError(error)
  return data
}
