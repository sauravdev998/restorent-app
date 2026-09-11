import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  MouseSensor,
  TouchSensor,
  useSensor,
  useSensors,
  type Announcements,
  type DragEndEvent,
  type Modifier,
  type UniqueIdentifier,
} from '@dnd-kit/core'
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { GripVertical } from 'lucide-react'
import { useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { cn } from '@/shared/ui/cn'
import { Icon } from '@/shared/ui/icon'
import { showToast } from '@/shared/ui/toast-store'

/** What is being ordered, which decides the words a screen reader hears. */
export type ReorderKind = 'dish' | 'category'

export interface ReorderableListProps<T extends { id: string }> {
  /** The items in the server's order. */
  items: readonly T[]
  /** What each item is called, for its handle's name and the announcements. */
  nameOf: (item: T) => string
  kind: ReorderKind
  /** Saves the complete new order. Rejects when the server refuses it. */
  save: (ids: string[]) => Promise<unknown>
  /** Renders one item's content, given the drag handle to put in it. */
  children: (item: T, handle: ReactNode) => ReactNode
  /** Classes for the list element itself. */
  className?: string
  /** Classes for each item's element. */
  itemClassName?: string
}

/**
 * A list an admin reorders by dragging, with a mouse, a finger, or the keyboard
 * alone, on dnd-kit underneath.
 *
 * **Only the handle drags.** The rest of each row stays clickable, so an edit
 * button or a switch inside a row is never the start of an accidental drag.
 * On touch the handle needs a long press, so scrolling a long menu with a thumb
 * never picks anything up.
 *
 * **Every word a screen reader hears is translated.** dnd-kit's own
 * announcements and its instructions are English; these are the admin's
 * language, naming the dish or category and its position out of how many.
 *
 * **The dropped order is held here while it saves, never in the query cache.**
 * That is spec 0007's rule of no cache optimism kept: the list shows where the
 * admin put things at once, and nothing is written into the cache ahead of the
 * server. On success the menus are refetched and the held order let go once
 * the refetch has landed. On refusal, `menu_changed` most often because a dish
 * was added or removed mid drag, the held order is dropped, the menus are
 * refetched, and the list shows the server's current order with a message
 * saying why, never the stale order held before the conflict.
 *
 * Nothing can be dragged out of this list into another one. A dish moves to
 * another category through its edit form, where its version is checked.
 */
export function ReorderableList<T extends { id: string }>({
  items,
  nameOf,
  kind,
  save,
  children,
  className,
  itemClassName,
}: ReorderableListProps<T>) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const [held, setHeld] = useState<string[] | null>(null)
  // What the dragged item was last announced as being over. dnd-kit reports
  // the item as over itself the instant it is picked up, which would bury the
  // pick up sentence under "moved to the position it is already in".
  const lastOver = useRef<UniqueIdentifier | null>(null)

  const sensors = useSensors(
    useSensor(MouseSensor, { activationConstraint: { distance: 4 } }),
    useSensor(TouchSensor, { activationConstraint: { delay: 250, tolerance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  )

  const reorder = useMutation({
    mutationFn: (ids: string[]) => save(ids),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
    },
    onError: async (error: unknown) => {
      setHeld(null)
      showToast({ title: apiErrorMessage(failureBody(error), common), tone: 'late' })
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
    },
    onSettled: () => {
      setHeld(null)
    },
  })

  const byId = new Map(items.map((item) => [item.id, item]))
  const serverIds = items.map((item) => item.id)
  // A held order naming something the server no longer has is not shown: the
  // server's list is the truth the moment the two disagree on membership.
  const ids =
    held !== null && held.length === serverIds.length && held.every((id) => byId.has(id))
      ? held
      : serverIds

  const name = (id: UniqueIdentifier): string => {
    const item = byId.get(String(id))
    return item ? nameOf(item) : ''
  }
  const position = (id: UniqueIdentifier): number => ids.indexOf(String(id)) + 1

  const announcements: Announcements = {
    onDragStart: ({ active }) => {
      lastOver.current = active.id
      return t('menu.order.pickedUp', {
        name: name(active.id),
        position: position(active.id),
        total: ids.length,
      })
    },
    onDragOver: ({ active, over }) => {
      const overId = over?.id ?? null
      if (overId === lastOver.current) return undefined
      lastOver.current = overId

      return over
        ? t('menu.order.movedTo', {
            name: name(active.id),
            position: position(over.id),
            total: ids.length,
          })
        : t('menu.order.outside', { name: name(active.id) })
    },
    onDragEnd: ({ active, over }) =>
      over
        ? t('menu.order.dropped', {
            name: name(active.id),
            position: position(over.id),
            total: ids.length,
          })
        : t('menu.order.droppedBack', { name: name(active.id) }),
    onDragCancel: ({ active }) =>
      t('menu.order.cancelled', {
        name: name(active.id),
        position: position(active.id),
        total: ids.length,
      }),
  }

  function onDragEnd({ active, over }: DragEndEvent): void {
    if (!over || active.id === over.id) return

    const from = ids.indexOf(String(active.id))
    const to = ids.indexOf(String(over.id))
    if (from === -1 || to === -1) return

    const next = arrayMove(ids, from, to)
    setHeld(next)
    reorder.mutate(next)
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      modifiers={[upAndDownOnly]}
      accessibility={{
        announcements,
        screenReaderInstructions: {
          draggable:
            kind === 'dish'
              ? t('menu.order.dishInstructions')
              : t('menu.order.categoryInstructions'),
        },
      }}
      onDragEnd={onDragEnd}
    >
      <SortableContext items={ids} strategy={verticalListSortingStrategy}>
        <ol className={className} aria-busy={reorder.isPending || undefined}>
          {ids.map((id) => {
            const item = byId.get(id)
            if (!item) return null

            return (
              <SortableItem
                key={id}
                id={id}
                label={t(kind === 'dish' ? 'menu.order.moveDish' : 'menu.order.moveCategory', {
                  name: nameOf(item),
                })}
                roleDescription={
                  kind === 'dish' ? t('menu.order.dishRole') : t('menu.order.categoryRole')
                }
                className={itemClassName}
              >
                {(handle) => children(item, handle)}
              </SortableItem>
            )
          })}
        </ol>
      </SortableContext>
    </DndContext>
  )
}

/**
 * Keeps a dragged row in its column.
 *
 * The lists are vertical, so sideways movement means nothing and only makes a
 * row look as if it could be dropped somewhere it cannot.
 */
const upAndDownOnly: Modifier = ({ transform }) => ({ ...transform, x: 0 })

interface SortableItemProps {
  id: string
  /** The handle's accessible name, such as "Move Paneer tikka". */
  label: string
  /** What a screen reader calls the handle instead of dnd-kit's English word. */
  roleDescription: string
  className?: string | undefined
  children: (handle: ReactNode) => ReactNode
}

/** One row in the list, and the handle that is the only part of it that drags. */
function SortableItem({ id, label, roleDescription, className, children }: SortableItemProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id, attributes: { roleDescription } })

  const style: CSSProperties = {
    transform: transform ? `translate3d(0, ${String(transform.y)}px, 0)` : undefined,
    transition,
  }

  const handle = (
    <button
      ref={setActivatorNodeRef}
      type="button"
      {...attributes}
      {...listeners}
      aria-label={label}
      className={cn(
        'target-min inline-flex shrink-0 cursor-grab touch-none items-center justify-center rounded-md',
        'text-muted-foreground hover:bg-accent hover:text-accent-foreground',
        isDragging && 'cursor-grabbing',
      )}
    >
      <Icon icon={GripVertical} size="md" />
    </button>
  )

  return (
    <li
      ref={setNodeRef}
      style={style}
      className={cn(
        className,
        // Lifted with a real border rather than a shade or transparency, so it
        // reads as picked up in forced colours too and never shows the row
        // beneath it through itself.
        isDragging && 'border-line relative z-10 rounded-md border-primary shadow-lg',
      )}
    >
      {children(handle)}
    </li>
  )
}
