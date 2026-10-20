import { Link } from '@tanstack/react-router'
import type { LucideIcon } from 'lucide-react'
import { ChevronRight } from 'lucide-react'
import type { ReactNode } from 'react'

import type { GuildPagePath } from '@/lib/guild'
import { toneClass } from '@/lib/tone'
import type { Tone } from '@/lib/tone'
import { cn } from '@/lib/utils'

/** Pages d'un serveur vers lesquelles mène une tuile. */
export type OverviewTarget =
  | GuildPagePath
  | '/serveurs/$guildId/presences'
  | '/serveurs/$guildId/absences'
  | '/serveurs/$guildId/recrues'

export type OverviewTileProps = {
  guildId: string
  title: string
  icon: LucideIcon
  /** `null` : pas de page ouverte au compte, la tuile n'est pas un lien. */
  to: OverviewTarget | '/tickets' | null
  value: ReactNode
  tone?: Tone
  hint?: ReactNode
  children?: ReactNode
}

const TILE =
  'group flex h-full flex-col rounded-xl border bg-card px-4 py-3.5 shadow-xs outline-none transition-colors focus-visible:ring-[3px] focus-visible:ring-ring/50'

/** Tuile de l'accueil : chiffre principal, précision, et lien vers la page du module. */
export const OverviewTile = ({
  guildId,
  title,
  icon: Icon,
  to,
  value,
  tone = 'default',
  hint,
  children,
}: OverviewTileProps) => {
  const content = (
    <>
      <div className="flex items-center gap-2 text-ink-muted">
        <Icon className="size-4 text-ink-subtle group-hover:text-brand" aria-hidden />
        <h2 className="text-xs font-medium tracking-wide text-ink-subtle uppercase">{title}</h2>
        {to && <ChevronRight className="ml-auto size-4 text-ink-subtle group-hover:text-brand" aria-hidden />}
      </div>
      <p className={cn('mt-2 text-2xl font-semibold tabular-nums', toneClass[tone])}>{value}</p>
      {hint && <p className="mt-0.5 text-sm text-ink-muted">{hint}</p>}
      {children && <div className="mt-3 text-sm">{children}</div>}
    </>
  )
  if (to === null) return <div className={TILE}>{content}</div>
  if (to === '/tickets') {
    return (
      <Link to="/tickets" className={cn(TILE, 'hover:border-line-strong')}>
        {content}
      </Link>
    )
  }
  return (
    <Link to={to} params={{ guildId }} className={cn(TILE, 'hover:border-line-strong')}>
      {content}
    </Link>
  )
}
