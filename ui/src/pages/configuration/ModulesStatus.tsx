import { useQuery } from '@tanstack/react-query'
import { CircleCheck, CircleMinus, TriangleAlert } from 'lucide-react'

import { configQuery } from '@/api/queries'
import type { ConfigStatus } from '@/api/types'
import { Badge } from '@/atoms/Badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/atoms/Card'
import { Skeleton } from '@/atoms/Skeleton'
import { useGuild } from '@/lib/guild-context'
import { CopyButton } from '@/molecules/CopyButton'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'

export const STATUS: Record<
  ConfigStatus['modules'][number]['status'],
  { label: string; variant: 'success' | 'warning' | 'neutral' }
> = {
  ok: { label: 'Prêt', variant: 'success' },
  incomplete: { label: 'Incomplet', variant: 'warning' },
  off: { label: 'Non utilisé', variant: 'neutral' },
}

export function ModulesStatus() {
  const { guild } = useGuild()
  const query = useQuery(configQuery(guild.id))
  return (
    <Section
      title="État des modules"
      description="Ce qui est prêt, ce qui manque, et la commande Discord pour corriger."
    >
      {query.isPending ? (
        <div className="grid gap-4 md:grid-cols-2">
          <Skeleton className="h-36 rounded-xl" />
          <Skeleton className="h-36 rounded-xl" />
        </div>
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {query.data.modules.map((module) => {
            const status = STATUS[module.status]
            return (
              <Card key={module.key} className="gap-3">
                <CardHeader className="flex flex-row items-center justify-between gap-2">
                  <CardTitle>{module.label}</CardTitle>
                  <Badge variant={status.variant}>{status.label}</Badge>
                </CardHeader>
                <CardContent>
                  <ul className="grid gap-2">
                    {module.checks.map((check) => (
                      <li key={check.label} className="flex items-start gap-2 text-sm">
                        {check.ok ? (
                          <CircleCheck className="mt-0.5 size-4 shrink-0 text-staff" aria-label="OK" />
                        ) : module.status === 'off' ? (
                          <CircleMinus className="mt-0.5 size-4 shrink-0 text-ink-subtle" aria-label="Non utilisé" />
                        ) : (
                          <TriangleAlert className="mt-0.5 size-4 shrink-0 text-internal" aria-label="À corriger" />
                        )}
                        <div className="min-w-0 flex-1">
                          <p className="text-ink">{check.label}</p>
                          {check.detail && <p className="text-xs text-ink-muted">{check.detail}</p>}
                          {!check.ok && check.fix && (
                            <div className="mt-1 flex items-center gap-1">
                              <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs text-ink">
                                {check.fix}
                              </code>
                              <CopyButton text={check.fix} label="Copier la commande" />
                            </div>
                          )}
                        </div>
                      </li>
                    ))}
                  </ul>
                </CardContent>
              </Card>
            )
          })}
        </div>
      )}
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Objectif de cotisation
