// Page provisoire des liens de l'accueil (présences, absences, recrues) en attendant leur page.
// À remplacer par la vraie page lors de l'intégration (même adresse).

import { Link, useRouterState } from '@tanstack/react-router'
import { Hourglass } from 'lucide-react'

import { Button } from '@/atoms/Button'
import { useGuild } from '@/lib/guild-context'
import { EmptyState } from '@/molecules/EmptyState'
import { PageHeader } from '@/molecules/PageHeader'

const TITLES: Record<string, string> = { presences: 'Présences', absences: 'Absences', recrues: 'Recrues' }

export const SoonPage = () => {
  const { guild } = useGuild()
  const segment = useRouterState({ select: (state) => state.location.pathname.split('/').at(-1) ?? '' })
  return (
    <>
      <PageHeader title={TITLES[segment] ?? 'Bientôt'} description={guild.name} />
      <EmptyState title="Cette page arrive bientôt" icon={<Hourglass />}>
        <p>En attendant, tout se gère depuis Discord.</p>
        <Button asChild variant="outline" size="sm" className="mt-4">
          <Link to="/serveurs/$guildId" params={{ guildId: guild.id }}>
            Retour à l'accueil
          </Link>
        </Button>
      </EmptyState>
    </>
  )
}
