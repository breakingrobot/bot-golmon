import type { GuildAccess } from '@/api/types'
import { cn, initials } from '@/lib/utils'

const SIZES = {
  sm: { box: 'size-6 rounded-md', text: 'text-[0.625rem]' },
  lg: { box: 'size-10 rounded-lg', text: 'text-sm' },
} as const

/** Icône du serveur Discord, ou ses initiales. `sm` : sélecteur ; `lg` : tuile de l'accueil. */
export function GuildIcon({ guild, size = 'lg' }: { guild: GuildAccess; size?: keyof typeof SIZES }) {
  const { box, text } = SIZES[size]
  return guild.iconUrl ? (
    <img src={guild.iconUrl} alt="" className={box} />
  ) : (
    <span
      className={cn('flex shrink-0 items-center justify-center bg-brand-soft font-semibold text-brand', box, text)}
      aria-hidden
    >
      {initials(guild.name)}
    </span>
  )
}
