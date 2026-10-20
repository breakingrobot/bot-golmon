// Tableau rendu en cartes sur les écrans étroits (sous `sm`) : le tableau est masqué (`hidden
// sm:flex` sur sa carte) et chaque ligne devient une carte avec ses champs en libellé / valeur.

import type { ReactNode } from 'react'

import { cn } from '@/lib/utils'

export type MobileCardsProps = {
  children: ReactNode
  label: string
  /** Dans une carte existante : lignes séparées par un trait plutôt que des cartes. */
  flush?: boolean
  className?: string
}

/** Liste des cartes, visible seulement sous `sm`. */
export const MobileCards = ({ children, label, flush = false, className }: MobileCardsProps) => (
  <ul
    aria-label={label}
    className={cn(
      'sm:hidden',
      flush ? 'divide-y [&>li]:rounded-none [&>li]:border-0 [&>li]:shadow-none' : 'space-y-2',
      className,
    )}
  >
    {children}
  </ul>
)

export type MobileCardField = { label: string; value: ReactNode }

export type MobileCardProps = {
  title: ReactNode
  /** À droite du titre : statut, montant, actions. */
  aside?: ReactNode
  fields?: MobileCardField[]
  children?: ReactNode
  className?: string
}

const NO_FIELDS: MobileCardField[] = []

export const MobileCard = ({ title, aside, fields = NO_FIELDS, children, className }: MobileCardProps) => (
  <li className={cn('relative rounded-xl border bg-card px-4 py-3 text-sm shadow-xs', className)}>
    <div className="flex items-start justify-between gap-3">
      <div className="min-w-0 font-medium text-ink">{title}</div>
      {aside && <div className="flex shrink-0 items-center gap-1">{aside}</div>}
    </div>
    {fields.length > 0 && (
      <dl className="mt-2 grid grid-cols-2 gap-x-4 gap-y-1.5">
        {fields.map((field) => (
          <div key={field.label} className="min-w-0">
            <dt className="text-xs text-ink-subtle">{field.label}</dt>
            <dd className="truncate text-ink">{field.value}</dd>
          </div>
        ))}
      </dl>
    )}
    {children}
  </li>
)
