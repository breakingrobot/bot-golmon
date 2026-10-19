import { CircleAlert, Lock, RotateCcw } from 'lucide-react'

import { ApiRequestError, errorMessage } from '@/api/client'
import { Button } from '@/atoms/Button'
import { EmptyState } from '@/molecules/EmptyState'
import { Forbidden } from '@/molecules/Forbidden'
import { NotFoundState } from '@/molecules/NotFoundState'

/** Erreur d'une requête : 403 → accès refusé, 404 → introuvable, sinon message + réessayer. */
export function QueryError({ error, retry }: { error: unknown; retry?: () => void }) {
  if (error instanceof ApiRequestError && error.status === 403) return <Forbidden />
  if (error instanceof ApiRequestError && error.status === 404) return <NotFoundState>{error.message}</NotFoundState>
  if (error instanceof ApiRequestError && error.status === 401) {
    return (
      <EmptyState title="Connexion requise" icon={<Lock />} className="mt-4">
        {error.loginUrl
          ? 'Redirection vers la connexion Discord…'
          : "La connexion Discord n'est pas configurée sur ce bot : seuls les logs de tickets sont lisibles, par leur lien."}
      </EmptyState>
    )
  }
  return (
    <div
      role="alert"
      className="mt-4 flex flex-wrap items-center gap-3 rounded-xl border border-danger/40 bg-danger-soft px-4 py-3 text-sm text-danger"
    >
      <CircleAlert className="size-4 shrink-0" aria-hidden />
      <span className="min-w-0 flex-1">{errorMessage(error)}</span>
      {retry && (
        <Button variant="outline" size="sm" onClick={retry}>
          <RotateCcw aria-hidden /> Réessayer
        </Button>
      )}
    </div>
  )
}
