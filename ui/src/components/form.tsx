// Champs de formulaire : libellé + contrôle + aide / erreur reliés pour l'accessibilité.

import { Download } from 'lucide-react'
import { useId, type ReactNode } from 'react'

import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { cn } from '@/lib/utils'

export function Field({
  label,
  hint,
  error,
  className,
  children,
}: {
  label: ReactNode
  hint?: ReactNode
  error?: string | null | undefined
  className?: string
  children: (props: { id: string; 'aria-describedby'?: string; 'aria-invalid'?: boolean }) => ReactNode
}) {
  const id = useId()
  const hintId = `${id}-aide`
  const errorId = `${id}-erreur`
  const describedBy = [hint ? hintId : null, error ? errorId : null].filter(Boolean).join(' ')
  return (
    <div className={cn('grid gap-1.5', className)}>
      <Label htmlFor={id} className="text-ink-muted">
        {label}
      </Label>
      {children({
        id,
        ...(describedBy ? { 'aria-describedby': describedBy } : {}),
        ...(error ? { 'aria-invalid': true } : {}),
      })}
      {hint && !error && (
        <p id={hintId} className="text-xs text-ink-subtle">
          {hint}
        </p>
      )}
      {error && (
        <p id={errorId} className="text-xs text-danger" role="alert">
          {error}
        </p>
      )}
    </div>
  )
}

/** Compteur de caractères « 12 / 80 ». */
export function CharCount({ value, max }: { value: string; max: number }) {
  return (
    <span className={cn('text-xs tabular-nums', value.length > max ? 'text-danger' : 'text-ink-subtle')}>
      {value.length} / {max}
    </span>
  )
}

export function CsvButton({ href }: { href: string }) {
  return (
    <Button asChild variant="outline" size="sm">
      <a href={href} download>
        <Download aria-hidden /> Exporter en CSV
      </a>
    </Button>
  )
}

export function FilterBar({ children, onSubmit }: { children: ReactNode; onSubmit: () => void }) {
  return (
    <form
      className="grid gap-3 rounded-xl border bg-card p-4 shadow-xs sm:grid-cols-2 lg:grid-cols-[repeat(auto-fit,minmax(10rem,1fr))]"
      onSubmit={(event) => {
        event.preventDefault()
        onSubmit()
      }}
    >
      {children}
    </form>
  )
}
