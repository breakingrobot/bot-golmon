import { Link } from '@tanstack/react-router'
import { SearchX } from 'lucide-react'
import type { ReactNode } from 'react'

import { Button } from '@/atoms/Button'
import { EmptyState } from '@/molecules/EmptyState'

export function NotFoundState({ children }: { children?: ReactNode }) {
  return (
    <EmptyState title="Page introuvable" icon={<SearchX />} className="mt-4">
      <p>{children ?? 'Cette adresse ne correspond à aucune page. Le lien est peut-être ancien.'}</p>
      <Button asChild variant="outline" size="sm" className="mt-4">
        <Link to="/">Retour à l'accueil</Link>
      </Button>
    </EmptyState>
  )
}
