import { useId } from 'react'
import type { ReactNode } from 'react'

import { Label } from '@/atoms/Label'
import { cn } from '@/lib/utils'

export type FieldProps = {
  label: ReactNode
  hint?: ReactNode
  error?: string | null | undefined
  className?: string
  children: (props: { id: string; 'aria-describedby'?: string; 'aria-invalid'?: boolean }) => ReactNode
}

export const Field = ({ label, hint, error, className, children }: FieldProps) => {
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
