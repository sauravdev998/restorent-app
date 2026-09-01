import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { describe, expect, it } from 'vitest'

import { expectAccessible } from '@/test/axe'

import { Button } from './button'
import { Dialog } from './dialog'
import { Field } from './field'
import { Input } from './input'
import { Select } from './select'

function SampleForm() {
  return (
    <>
      <Field label="Dish name" hint="Shown to the kitchen" required>
        <Input />
      </Field>
      <Field label="Course">
        <Select>
          <option value="main">Main</option>
        </Select>
      </Field>
      <Field label="Notes" error="A note is required.">
        <Input />
      </Field>
    </>
  )
}

describe('Field', () => {
  it('is accessible in both appearances and all three densities', async () => {
    await expectAccessible(<SampleForm />)
  })

  it('binds the label to the control', () => {
    render(
      <Field label="Dish name">
        <Input />
      </Field>,
    )
    // getByLabelText only finds it if the label is programmatically bound, so
    // this failing means a screen reader would announce an unnamed text box.
    expect(screen.getByLabelText('Dish name')).toBeInstanceOf(HTMLInputElement)
  })

  it('marks an invalid control and links its message', () => {
    render(
      <Field label="Notes" hint="Kept short" error="A note is required.">
        <Input />
      </Field>,
    )

    const input = screen.getByLabelText('Notes')
    expect(input).toHaveAttribute('aria-invalid', 'true')

    const describedBy = input.getAttribute('aria-describedby')?.split(' ') ?? []
    const described = describedBy.map((id) => document.getElementById(id)?.textContent)
    expect(described).toContain('Kept short')
    expect(described).toContain('A note is required.')
  })

  it('announces the error rather than only showing it', () => {
    render(
      <Field label="Notes" error="A note is required.">
        <Input />
      </Field>,
    )
    expect(screen.getByRole('alert')).toHaveTextContent('A note is required.')
  })

  it('refuses a control used without it', () => {
    // A control with no Field has no label bound to it, which is the single most
    // common way a form becomes unusable with a screen reader.
    expect(() => render(<Input />)).toThrow(/inside a <Field>/)
  })
})

describe('Dialog', () => {
  function Harness() {
    const [open, setOpen] = useState(false)
    return (
      <>
        <Button
          onClick={() => {
            setOpen(true)
          }}
        >
          Open
        </Button>
        <Dialog
          open={open}
          onOpenChange={setOpen}
          title="Send this round"
          description="To the kitchen"
        >
          <Button>Inside</Button>
        </Dialog>
      </>
    )
  }

  it('is accessible, including the part that renders into a portal', async () => {
    await expectAccessible(
      <Dialog
        open
        onOpenChange={() => undefined}
        title="Send this round"
        description="To the kitchen"
      />,
      { portal: true },
    )
  })

  it('holds focus, closes on Escape, and hands focus back to what opened it', async () => {
    const user = userEvent.setup()
    render(<Harness />)

    const opener = screen.getByRole('button', { name: 'Open' })
    await user.click(opener)

    const dialog = await screen.findByRole('dialog')
    expect(dialog).toHaveAccessibleName('Send this round')
    expect(dialog).toHaveAccessibleDescription('To the kitchen')
    // Focus moved into the dialog rather than staying on the page behind it.
    expect(dialog.contains(document.activeElement)).toBe(true)

    await user.keyboard('{Escape}')

    expect(screen.queryByRole('dialog')).toBeNull()
    // Restored asynchronously, after the focus scope unwinds.
    await waitFor(() => {
      expect(document.activeElement).toBe(opener)
    })
  })
})
