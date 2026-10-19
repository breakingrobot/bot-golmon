// Ancienne adresse (favoris) : /stats → accueil.

import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/stats/')({
  beforeLoad: () => {
    throw redirect({ to: '/', replace: true })
  },
})
