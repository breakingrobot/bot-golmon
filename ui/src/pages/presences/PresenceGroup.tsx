import { RefreshCw } from 'lucide-react'

import type { MemberRef, PresenceRespondent } from '@/api/types'
import { formatNumber } from '@/lib/format'
import { cn } from '@/lib/utils'
import { MemberName } from '@/molecules/MemberName'

export type PresenceGroupProps = {
  title: string
  /** Classe de couleur du titre. */
  tone: string
  people: (PresenceRespondent | MemberRef)[]
  guildId: string | undefined
}

/** Membres d'une réponse (ou sans réponse), avec retard en minutes et changement d'avis. */
export const PresenceGroup = ({ title, tone, people, guildId }: PresenceGroupProps) => (
  <section>
    <h3 className={cn('text-xs font-medium tracking-wide uppercase', tone)}>
      {title} · {formatNumber(people.length)}
    </h3>
    {people.length === 0 ? (
      <p className="mt-1 text-sm text-ink-subtle">Personne.</p>
    ) : (
      <ul className="mt-2 flex flex-wrap gap-x-4 gap-y-2">
        {people.map((person) => {
          const member = 'member' in person ? person.member : person
          const respondent = 'member' in person ? person : null
          return (
            <li key={member.id} className="flex min-w-0 items-center gap-1.5 text-sm">
              <MemberName member={member} guildId={guildId} className="text-sm" />
              {respondent && respondent.retardMinutes !== null && (
                <span className="text-xs text-internal">+{formatNumber(respondent.retardMinutes)} min</span>
              )}
              {respondent?.changed && (
                <RefreshCw className="size-3.5 text-ink-subtle" aria-label="A changé d'avis" role="img" />
              )}
            </li>
          )
        })}
      </ul>
    )}
  </section>
)
