import type { QueryClient } from '@tanstack/react-query'
import { createRouter } from '@tanstack/react-router'
import type { RouterHistory } from '@tanstack/react-router'

import { PageSkeleton } from '@/molecules/PageSkeleton'
import { routeTree } from '@/routeTree.gen'

/**
 * Paramètres d'adresse classiques (`?jours=7&page=2`) plutôt que le JSON par
 * défaut du routeur (`?jours=%227%22`) : liens lisibles, identiques à ceux que
 * le bot envoie sur Discord. Chaque page valide ses paramètres (`validateSearch`).
 */
const parseSearch = (search: string): Record<string, unknown> => Object.fromEntries(new URLSearchParams(search))

const stringifySearch = (search: Record<string, unknown>): string => {
  const params = new URLSearchParams()
  for (const [key, value] of Object.entries(search)) {
    if (value === undefined || value === null || value === '') continue
    params.set(key, typeof value === 'string' ? value : JSON.stringify(value))
  }
  const text = params.toString()
  return text ? `?${text}` : ''
}

/** `history` : historique en mémoire pour les tests ; par défaut, celui du navigateur. */
export const createAppRouter = (queryClient: QueryClient, history?: RouterHistory) =>
  createRouter({
    ...(history ? { history } : {}),
    routeTree,
    context: { queryClient },
    parseSearch,
    stringifySearch,
    defaultPreload: 'intent',
    // Les données viennent de TanStack Query : le routeur ne garde pas de cache de loaders.
    defaultPreloadStaleTime: 0,
    defaultPendingComponent: () => <PageSkeleton />,
    scrollRestoration: true,
  })

// Fusion de déclarations : seule une interface peut compléter `Register` du routeur.
declare module '@tanstack/react-router' {
  // oxlint-disable-next-line typescript/consistent-type-definitions
  interface Register {
    router: ReturnType<typeof createAppRouter>
  }
}
