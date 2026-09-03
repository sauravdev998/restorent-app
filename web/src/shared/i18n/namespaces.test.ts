import { describe, expect, it } from 'vitest'

import { SURFACES } from '@/shared/surface'

import { DEFAULT_NAMESPACE, NAMESPACES, namespacesForSurface } from './namespaces'

/**
 * The split, which is the whole reason a waiter's phone downloads less than an
 * admin's desktop. A namespace list that quietly widens is a download nobody
 * notices until they are on one bar in a basement kitchen.
 */

describe('namespacesForSurface', () => {
  it('asks for the shell and the surface, and nothing else', () => {
    expect(namespacesForSurface('waiter')).toEqual(['common', 'waiter'])
    expect(namespacesForSurface('admin')).toEqual(['common', 'admin'])
    expect(namespacesForSurface('kitchen')).toEqual(['common', 'kitchen'])
  }) // covers: AC-5

  it('never carries another surface’s vocabulary', () => {
    // The assertion that actually protects the split: it fails the moment
    // somebody adds a namespace here "just so it is always there".
    for (const surface of SURFACES) {
      const asked = namespacesForSurface(surface)
      const others = SURFACES.filter((other) => other !== surface)

      for (const other of others) {
        expect(asked).not.toContain(other)
      }
    }
  }) // covers: AC-5

  it('always includes the shell, because every surface renders it', () => {
    for (const surface of SURFACES) {
      expect(namespacesForSurface(surface)).toContain(DEFAULT_NAMESPACE)
    }
  }) // covers: AC-5

  it('asks for exactly two files per surface', () => {
    // Two, so a language switch on the waiter screen waits on two requests and
    // never on the admin file.
    for (const surface of SURFACES) {
      expect(namespacesForSurface(surface)).toHaveLength(2)
    }
  }) // covers: AC-5
})

describe('NAMESPACES', () => {
  it('holds the shell namespace a bare t() reads from', () => {
    expect(NAMESPACES).toContain(DEFAULT_NAMESPACE)
  }) // covers: AC-5

  it('gives every surface a namespace of its own name', () => {
    // The link between the two lists is by name rather than by a mapping, so a
    // fourth surface added without its namespace file fails here rather than
    // rendering raw keys on a real screen.
    for (const surface of SURFACES) {
      expect(NAMESPACES).toContain(surface)
    }
  }) // covers: AC-5

  it('carries no namespace that nothing asks for', () => {
    const asked = new Set(SURFACES.flatMap(namespacesForSurface))
    expect([...NAMESPACES].filter((namespace) => !asked.has(namespace))).toEqual([])
  }) // covers: AC-5
})
