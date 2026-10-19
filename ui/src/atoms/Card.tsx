import type { ComponentProps } from 'react'

import { cn } from '@/lib/utils'

export const Card = ({ className, ...props }: ComponentProps<'div'>) => (
  <div
    data-slot="card"
    className={cn('flex flex-col gap-4 rounded-xl border bg-card py-5 text-card-foreground shadow-xs', className)}
    {...props}
  />
)

export const CardHeader = ({ className, ...props }: ComponentProps<'div'>) => (
  <div
    data-slot="card-header"
    className={cn(
      'grid auto-rows-min grid-rows-[auto_auto] items-start gap-1 px-5 has-data-[slot=card-action]:grid-cols-[1fr_auto]',
      className,
    )}
    {...props}
  />
)

export const CardTitle = ({ className, children, ...props }: ComponentProps<'h2'>) => (
  <h2 data-slot="card-title" className={cn('leading-tight font-semibold', className)} {...props}>
    {children}
  </h2>
)

export const CardDescription = ({ className, ...props }: ComponentProps<'p'>) => (
  <p data-slot="card-description" className={cn('text-sm text-muted-foreground', className)} {...props} />
)

export const CardAction = ({ className, ...props }: ComponentProps<'div'>) => (
  <div
    data-slot="card-action"
    className={cn('col-start-2 row-span-2 row-start-1 self-start justify-self-end', className)}
    {...props}
  />
)

export const CardContent = ({ className, ...props }: ComponentProps<'div'>) => (
  <div data-slot="card-content" className={cn('px-5', className)} {...props} />
)

export const CardFooter = ({ className, ...props }: ComponentProps<'div'>) => (
  <div data-slot="card-footer" className={cn('flex items-center px-5', className)} {...props} />
)
