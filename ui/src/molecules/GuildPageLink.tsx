import { Link } from '@tanstack/react-router'
import type { ReactNode } from 'react'

import type { GuildPage } from '@/lib/guild'

/** Lien vers une page d'un serveur (la fiche pointe vers le membre connecté). */
export function GuildPageLink({
  page,
  guildId,
  userId,
  className,
  activeClassName,
  onNavigate,
  children,
}: {
  page: GuildPage
  guildId: string
  userId: string
  className?: string
  activeClassName?: string
  onNavigate?: () => void
  children: ReactNode
}) {
  const activeProps = activeClassName ? { className: activeClassName } : {}
  const common = {
    className,
    activeProps,
    activeOptions: { exact: page.key === 'stats' || page.key === 'membres' },
    onClick: onNavigate,
  }
  if (page.to === null) {
    return (
      <Link to="/serveurs/$guildId/membres/$userId" params={{ guildId, userId }} {...common}>
        {children}
      </Link>
    )
  }
  return (
    <Link to={page.to} params={{ guildId }} {...common}>
      {children}
    </Link>
  )
}
