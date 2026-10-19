import { Link } from '@tanstack/react-router'
import { Lock } from 'lucide-react'
import type { ReactNode } from 'react'

import { Button } from '@/atoms/Button'
import { EmptyState } from '@/molecules/EmptyState'

export type ForbiddenProps = { children?: ReactNode }

export const Forbidden = ({ children }: ForbiddenProps) => (
  <EmptyState title="Accès refusé" icon={<Lock />} className="mt-4">
    {children ?? (
      <p>
        Votre rôle ne donne pas accès à cette page. Les niveaux se règlent sur Discord avec{' '}
        <code className="rounded bg-muted px-1 font-mono text-xs">/config permission</code>.
      </p>
    )}
    <Button asChild variant="outline" size="sm" className="mt-4">
      <Link to="/">Retour à l'accueil</Link>
    </Button>
  </EmptyState>
)
