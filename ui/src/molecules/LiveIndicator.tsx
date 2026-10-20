import type { LiveStatus } from '@/hooks/use-live-updates'
import { cn } from '@/lib/utils'

const LABELS: Record<LiveStatus, string> = {
  connecting: 'Connexion…',
  live: 'En direct',
  retrying: 'Reconnexion…',
  unavailable: 'Hors direct',
}

const TITLES: Record<LiveStatus, string> = {
  connecting: 'Connexion au flux du serveur.',
  live: 'Les pages se mettent à jour toutes seules quand quelque chose change sur Discord ou sur le site.',
  retrying: 'Connexion perdue : nouvel essai en cours. Les pages restent utilisables.',
  unavailable: 'Mise à jour en direct indisponible : recharger la page pour voir les derniers changements.',
}

export type LiveIndicatorProps = { status: LiveStatus; className?: string }

/** Pastille discrète : la page se met-elle à jour toute seule ? */
export const LiveIndicator = ({ status, className }: LiveIndicatorProps) => (
  <span
    role="status"
    title={TITLES[status]}
    className={cn('inline-flex items-center gap-1.5 text-xs text-ink-subtle', className)}
  >
    <span
      aria-hidden
      className={cn(
        'size-2 rounded-full',
        status === 'live' && 'bg-staff motion-safe:animate-pulse',
        status === 'retrying' && 'bg-internal',
        (status === 'connecting' || status === 'unavailable') && 'bg-line-strong',
      )}
    />
    {LABELS[status]}
  </span>
)
