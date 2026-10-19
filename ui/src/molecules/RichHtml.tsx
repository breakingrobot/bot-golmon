// HTML assaini par le serveur (markdown Discord) : heures locales et spoilers cliquables.

import type { KeyboardEvent, MouseEvent } from 'react'

import { formatRelative } from '@/lib/format'
import { cn } from '@/lib/utils'

export const FORMATS: Record<string, Intl.DateTimeFormatOptions> = {
  t: { timeStyle: 'short' },
  T: { timeStyle: 'medium' },
  d: { dateStyle: 'short' },
  D: { dateStyle: 'long' },
  f: { dateStyle: 'long', timeStyle: 'short' },
  F: { dateStyle: 'full', timeStyle: 'short' },
}

export const localizeTimes = (root: HTMLElement) => {
  for (const element of root.querySelectorAll<HTMLTimeElement>('time[data-local]')) {
    const date = new Date(element.dateTime)
    if (Number.isNaN(date.getTime())) continue
    const style = element.dataset['format'] ?? 'f'
    element.textContent =
      style === 'R'
        ? formatRelative(date.toISOString())
        : new Intl.DateTimeFormat('fr-FR', FORMATS[style] ?? FORMATS['f']).format(date)
    element.title = date.toLocaleString('fr-FR')
  }
}

export const reveal = (target: EventTarget) => {
  if (target instanceof Element) target.closest('.spoiler')?.classList.add('revealed')
}

export type RichHtmlProps = { html: string; className?: string }

export const RichHtml = ({ html, className }: RichHtmlProps) => (
  // Les spoilers (span tabindex=0 rendus par le serveur) se révèlent au clic ou au clavier.
  // oxlint-disable-next-line jsx-a11y/no-static-element-interactions, jsx-a11y/click-events-have-key-events
  <div
    // Heures locales appliquées à chaque rendu du HTML (callback ref, pas d'état).
    ref={(node) => {
      if (node) localizeTimes(node)
    }}
    className={cn('message-body', className)}
    onClick={(event: MouseEvent) => reveal(event.target)}
    onKeyDown={(event: KeyboardEvent) => {
      if (event.key === 'Enter' || event.key === ' ') reveal(event.target)
    }}
    // HTML assaini côté serveur (contrat : `bodyHtml`, `detailsHtml`).
    dangerouslySetInnerHTML={{ __html: html }}
  />
)
