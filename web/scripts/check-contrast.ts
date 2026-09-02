/**
 * The contrast gate.
 *
 * Reads the real colour values out of `src/styles/index.css`, resolves every
 * pair declared in `contrast-pairs.ts` in every appearance the stylesheet
 * defines, and fails the build if one drops below its threshold.
 *
 * It holds no colour of its own, and that is the whole design. A script with a
 * copied hex value passes happily on stale numbers the first time somebody
 * edits the palette, which is precisely the moment a gate is supposed to fire.
 *
 * Run by `pnpm check`. Run it alone with `pnpm --filter web contrast`.
 */
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import postcss, { type Container } from 'postcss'

import { CONTRAST_PAIRS, THRESHOLDS, type ContrastPair } from './contrast-pairs.ts'

/** The appearances the stylesheet builds, and which the gate therefore checks. */
const APPEARANCES = ['dark', 'light', 'print'] as const
type Appearance = (typeof APPEARANCES)[number]

/**
 * Forced colours is deliberately absent.
 *
 * Under `forced-colors: active` every role resolves to an operating system
 * keyword (`Canvas`, `CanvasText`, `Highlight`), and the operating system
 * guarantees those pairs itself. Measuring them here would mean inventing
 * values nobody actually renders. What that appearance needs instead is that
 * boundaries and focus rings survive at all, which is a structural rule the
 * components hold and the /design route proves.
 */
const STYLESHEET = resolve(dirname(fileURLToPath(import.meta.url)), '../src/styles/index.css')

type Scope = Map<string, string>

function collect(container: Container, scopes: Scope[]): void {
  container.each((node) => {
    if (node.type === 'decl' && node.prop.startsWith('--')) {
      for (const scope of scopes) scope.set(node.prop.slice(2), node.value.trim())
    }
  })
}

/**
 * Walks the stylesheet once, in document order, building one variable map per
 * appearance.
 *
 * Document order is what makes this correct rather than clever: the palette
 * comes first and lands in every appearance, the dark roles follow and land in
 * every appearance, then the light block overwrites the light map and the print
 * block overwrites the print map. Exactly the cascade a browser would apply.
 */
function readAppearances(css: string): Record<Appearance, Scope> {
  const scopes: Record<Appearance, Scope> = { dark: new Map(), light: new Map(), print: new Map() }
  const all = [scopes.dark, scopes.light, scopes.print]

  postcss.parse(css).each((node) => {
    if (node.type === 'rule') {
      if (node.selector === ':root') collect(node, all)
      else if (node.selector === "[data-theme='dark']") collect(node, [scopes.dark])
      else if (node.selector === "[data-theme='light']") collect(node, [scopes.light])
      return
    }

    if (node.type !== 'atrule' || node.name !== 'media') return

    // Forced colours resolves to system keywords, which this gate does not own.
    if (node.params.includes('forced-colors')) return

    const target = node.params.includes('prefers-color-scheme: light')
      ? scopes.light
      : node.params.includes('print')
        ? scopes.print
        : null
    if (!target) return

    node.each((inner) => {
      if (inner.type === 'rule') collect(inner, [target])
    })
  })

  return scopes
}

/** Follows a `var(--x)` chain down to a literal, refusing to loop forever. */
function resolveValue(name: string, scope: Scope, seen = new Set<string>()): string | null {
  if (seen.has(name)) return null
  seen.add(name)

  const raw = scope.get(name)
  if (raw === undefined) return null

  const reference = /^var\(\s*--([\w-]+)\s*\)$/.exec(raw)
  if (reference?.[1] !== undefined) return resolveValue(reference[1], scope, seen)

  return raw
}

function toRgb(value: string): [number, number, number] | null {
  const short = /^#([\da-f])([\da-f])([\da-f])$/i.exec(value)
  if (short) {
    return [short[1], short[2], short[3]].map((part) =>
      parseInt(`${part ?? ''}${part ?? ''}`, 16),
    ) as [number, number, number]
  }

  const long = /^#([\da-f]{2})([\da-f]{2})([\da-f]{2})$/i.exec(value)
  if (long) {
    return [long[1], long[2], long[3]].map((part) => parseInt(part ?? '', 16)) as [
      number,
      number,
      number,
    ]
  }

  return null
}

function channel(value: number): number {
  const c = value / 255
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
}

function luminance([r, g, b]: [number, number, number]): number {
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

function contrast(a: [number, number, number], b: [number, number, number]): number {
  const [lighter, darker] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return ((lighter ?? 0) + 0.05) / ((darker ?? 0) + 0.05)
}

interface Failure {
  appearance: Appearance
  pair: ContrastPair
  detail: string
}

function main(): void {
  const scopes = readAppearances(readFileSync(STYLESHEET, 'utf8'))
  const failures: Failure[] = []
  let checked = 0

  for (const appearance of APPEARANCES) {
    const scope = scopes[appearance]

    for (const pair of CONTRAST_PAIRS) {
      const onValue = resolveValue(pair.on, scope)
      const againstValue = resolveValue(pair.against, scope)

      if (onValue === null || againstValue === null) {
        failures.push({
          appearance,
          pair,
          detail: `--${onValue === null ? pair.on : pair.against} is not defined in this appearance`,
        })
        continue
      }

      const on = toRgb(onValue)
      const against = toRgb(againstValue)

      if (!on || !against) {
        failures.push({
          appearance,
          pair,
          detail: `not a colour this gate can measure: ${onValue} on ${againstValue}`,
        })
        continue
      }

      checked += 1
      const ratio = contrast(on, against)
      const threshold = THRESHOLDS[pair.kind]

      if (ratio < threshold) {
        failures.push({
          appearance,
          pair,
          detail: `${ratio.toFixed(2)}:1, needs ${String(threshold)}:1 (${onValue} on ${againstValue})`,
        })
      }
    }
  }

  if (failures.length > 0) {
    console.error(`\nContrast check failed. ${String(failures.length)} pair(s) below threshold:\n`)
    for (const { appearance, pair, detail } of failures) {
      console.error(`  [${appearance}] --${pair.on} on --${pair.against} (${pair.kind})`)
      console.error(`      ${pair.note}`)
      console.error(`      ${detail}\n`)
    }
    console.error('Adjust the colour in src/styles/index.css. Never lower the threshold.\n')
    process.exit(1)
  }

  console.log(
    `Contrast check passed: ${String(checked)} pairs across ${String(APPEARANCES.length)} appearances.`,
  )
}

main()
