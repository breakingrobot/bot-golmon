// Briques de mise en page communes : en-tête, sections, chiffres clés, états vides, erreurs, pagination.

import { Link } from '@tanstack/react-router'
import { ChevronLeft, ChevronRight, CircleAlert, Lock, RotateCcw, SearchX } from 'lucide-react'
import type { ReactNode } from 'react'

import { ApiRequestError, errorMessage } from '@/api/client'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/misc'
import { formatNumber } from '@/lib/format'
import { cn } from '@/lib/utils'

export function PageHeader({
  title,
  description,
  actions,
  children,
}: {
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
  children?: ReactNode
}) {
  return (
    <header className="mb-6 flex flex-wrap items-end justify-between gap-x-6 gap-y-3">
      <div className="min-w-0">
        <h1 className="text-2xl font-semibold tracking-tight text-ink">{title}</h1>
        {description && <p className="mt-1 text-sm text-ink-muted">{description}</p>}
        {children}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </header>
  )
}

export function Section({
  title,
  description,
  actions,
  children,
  className,
  id,
}: {
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
  children: ReactNode
  className?: string
  id?: string
}) {
  const headingId = id ? `${id}-titre` : undefined
  return (
    <section className={cn('mt-10 first:mt-0', className)} aria-labelledby={headingId} id={id}>
      <div className="mb-3 flex flex-wrap items-end justify-between gap-x-4 gap-y-2">
        <div>
          <h2 id={headingId} className="text-lg font-semibold text-ink">
            {title}
          </h2>
          {description && <p className="mt-0.5 text-sm text-ink-muted">{description}</p>}
        </div>
        {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
      </div>
      {children}
    </section>
  )
}

export type Tone = 'default' | 'user' | 'staff' | 'internal' | 'danger' | 'brand' | 'muted'

const toneClass: Record<Tone, string> = {
  default: 'text-ink',
  user: 'text-user',
  staff: 'text-staff',
  internal: 'text-internal',
  danger: 'text-danger',
  brand: 'text-brand',
  muted: 'text-ink-muted',
}

export function StatGrid({ children, className }: { children: ReactNode; className?: string }) {
  return <dl className={cn('grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4', className)}>{children}</dl>
}

export function StatCard({
  label,
  value,
  hint,
  tone = 'default',
  className,
}: {
  label: ReactNode
  value: ReactNode
  hint?: ReactNode
  tone?: Tone
  className?: string
}) {
  return (
    <div className={cn('rounded-xl border bg-card px-4 py-3 shadow-xs', className)}>
      <dt className="text-xs font-medium tracking-wide text-ink-subtle uppercase">{label}</dt>
      <dd className={cn('mt-1 text-xl font-semibold tabular-nums', toneClass[tone])}>{value}</dd>
      {hint && <dd className="mt-0.5 text-xs text-ink-muted">{hint}</dd>}
    </div>
  )
}

export function EmptyState({
  title,
  children,
  icon,
  className,
}: {
  title: ReactNode
  children?: ReactNode
  icon?: ReactNode
  className?: string
}) {
  return (
    <div
      className={cn(
        'flex flex-col items-center rounded-xl border border-dashed border-line-strong px-6 py-10 text-center',
        className,
      )}
    >
      {icon && <div className="mb-3 text-ink-subtle [&_svg]:size-6">{icon}</div>}
      <p className="font-medium text-ink">{title}</p>
      {children && <div className="mt-1 max-w-md text-sm text-ink-muted">{children}</div>}
    </div>
  )
}

export function Forbidden({ children }: { children?: ReactNode }) {
  return (
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
}

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

export function PageSkeleton({ stats = 4, rows = 6 }: { stats?: number; rows?: number }) {
  return (
    <div aria-busy="true" aria-label="Chargement">
      <Skeleton className="h-8 w-56" />
      <Skeleton className="mt-2 h-4 w-72" />
      <div className="mt-6 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
        {Array.from({ length: stats }, (_, index) => (
          <Skeleton key={index} className="h-20 rounded-xl" />
        ))}
      </div>
      <Skeleton className="mt-8 h-56 rounded-xl" />
      <div className="mt-8 space-y-2">
        {Array.from({ length: rows }, (_, index) => (
          <Skeleton key={index} className="h-10" />
        ))}
      </div>
    </div>
  )
}

export function TableSkeleton({ rows = 6 }: { rows?: number }) {
  return (
    <Card className="gap-2 px-5" aria-busy="true" aria-label="Chargement">
      {Array.from({ length: rows }, (_, index) => (
        <Skeleton key={index} className="h-8" />
      ))}
    </Card>
  )
}

export function Pagination({
  page,
  pageSize,
  total,
  onPage,
  newerLabel = 'Précédent',
  olderLabel = 'Suivant',
}: {
  page: number
  pageSize: number
  total: number
  onPage: (page: number) => void
  newerLabel?: string
  olderLabel?: string
}) {
  const pages = Math.max(1, Math.ceil(total / Math.max(pageSize, 1)))
  if (pages <= 1) return null
  return (
    <nav className="mt-4 flex items-center justify-between gap-3 text-sm" aria-label="Pagination">
      <Button variant="outline" size="sm" disabled={page <= 1} onClick={() => onPage(page - 1)}>
        <ChevronLeft aria-hidden /> {newerLabel}
      </Button>
      <span className="text-ink-muted tabular-nums">
        Page {formatNumber(page)} sur {formatNumber(pages)}
      </span>
      <Button variant="outline" size="sm" disabled={page >= pages} onClick={() => onPage(page + 1)}>
        {olderLabel} <ChevronRight aria-hidden />
      </Button>
    </nav>
  )
}

/** Barre de part (pourcentage), couleur selon seuils optionnels. */
export function ShareBar({
  value,
  max = 100,
  label,
  tone = 'brand',
  className,
}: {
  value: number
  max?: number
  label: string
  tone?: 'brand' | 'staff' | 'internal' | 'danger'
  className?: string
}) {
  const percent = max > 0 ? Math.max(0, Math.min(100, (value / max) * 100)) : 0
  const fill = { brand: 'bg-brand', staff: 'bg-staff', internal: 'bg-internal', danger: 'bg-danger' }[tone]
  return (
    <div
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={value}
      className={cn('h-2 w-24 overflow-hidden rounded-full bg-muted', className)}
    >
      <div className={cn('h-full rounded-full', fill)} style={{ width: `${percent}%` }} />
    </div>
  )
}

export function attendanceTone(percent: number): 'staff' | 'internal' | 'danger' {
  if (percent >= 75) return 'staff'
  if (percent >= 50) return 'internal'
  return 'danger'
}
