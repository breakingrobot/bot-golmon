// Ancienne adresse : /logs (liste des tickets) → /tickets, recherche conservée.

import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/logs/')({
  beforeLoad: ({ location }) => {
    throw redirect({ to: '/tickets', search: location.search as Record<string, never>, replace: true })
  },
})
