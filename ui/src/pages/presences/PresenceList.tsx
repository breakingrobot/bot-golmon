import { useQuery } from '@tanstack/react-query'
import { CalendarCheck, ChevronRight, X } from 'lucide-react'

import { presencesQuery } from '@/api/queries-groupe'
import type { PresenceFilters } from '@/api/queries-groupe'
import { Badge } from '@/atoms/Badge'
import { Button } from '@/atoms/Button'
import { Card } from '@/atoms/Card'
import { Input } from '@/atoms/Input'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { formatDateTime } from '@/lib/format'
import { presenceState, STATE_LABELS } from '@/lib/presence'
import { compact } from '@/lib/search'
import type { SearchPatch } from '@/lib/search'
import { cn } from '@/lib/utils'
import { EmptyState } from '@/molecules/EmptyState'
import { Field } from '@/molecules/Field'
import { Pagination } from '@/molecules/Pagination'
import { QueryError } from '@/molecules/QueryError'
import { TableSkeleton } from '@/molecules/TableSkeleton'
import { AnswerCounts } from '@/pages/presences/AnswerCounts'
import { STATES } from '@/pages/presences/presences-search'
import type { PresencesSearch, State } from '@/pages/presences/presences-search'

const ALL_STATES = 'toutes'

export type PresenceListProps = {
  guildId: string
  search: PresencesSearch
  onSearch: (patch: SearchPatch<PresencesSearch>) => void
  onOpen: (id: string) => void
}

export const PresenceList = ({ guildId, search, onSearch, onOpen }: PresenceListProps) => {
  const filters: PresenceFilters = compact({ etat: search.etat, du: search.du, au: search.au })
  const query = useQuery(presencesQuery(guildId, filters, search.page ?? 1))
  const filtered = Boolean(search.etat || search.du || search.au)

  return (
    <>
      <div className="grid gap-3 rounded-xl border bg-card p-4 shadow-xs sm:grid-cols-2 lg:grid-cols-[auto_1fr_1fr_auto] lg:items-end">
        <div className="grid gap-1.5">
          <span className="text-sm font-medium text-ink-muted" id="presences-etat">
            État
          </span>
          <ToggleGroup
            type="single"
            value={search.etat ?? ALL_STATES}
            onValueChange={(value) => {
              if (value) onSearch({ etat: value === ALL_STATES ? undefined : (value as State), page: undefined })
            }}
            aria-labelledby="presences-etat"
          >
            <ToggleGroupItem value={ALL_STATES}>Toutes</ToggleGroupItem>
            {STATES.map((state) => (
              <ToggleGroupItem key={state.value} value={state.value}>
                {state.label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </div>
        <Field label="Du">
          {(props) => (
            <Input
              {...props}
              type="date"
              value={search.du ?? ''}
              onChange={(event) => onSearch({ du: event.target.value || undefined, page: undefined })}
            />
          )}
        </Field>
        <Field label="Au">
          {(props) => (
            <Input
              {...props}
              type="date"
              value={search.au ?? ''}
              onChange={(event) => onSearch({ au: event.target.value || undefined, page: undefined })}
            />
          )}
        </Field>
        <div className="flex items-end">
          {filtered && (
            <Button
              variant="ghost"
              onClick={() => onSearch({ etat: undefined, du: undefined, au: undefined, page: undefined })}
            >
              <X aria-hidden /> Réinitialiser
            </Button>
          )}
        </div>
      </div>

      <section className="mt-6" aria-label="Liste des présences">
        {query.isPending ? (
          <TableSkeleton />
        ) : query.isError ? (
          <QueryError error={query.error} retry={() => void query.refetch()} />
        ) : query.data.items.length === 0 ? (
          <EmptyState title="Aucune présence" icon={<CalendarCheck />}>
            {filtered ? 'Aucune présence ne correspond aux filtres.' : "Aucune présence n'a encore été créée."}
          </EmptyState>
        ) : (
          <>
            <Card className={cn('gap-0 py-0', query.isPlaceholderData && 'opacity-60')}>
              <ul className="divide-y">
                {query.data.items.map((presence) => {
                  const state = STATE_LABELS[presenceState(presence)]
                  return (
                    <li key={presence.id}>
                      <button
                        type="button"
                        onClick={() => onOpen(presence.id)}
                        aria-haspopup="dialog"
                        className="flex w-full flex-wrap items-center gap-x-3 gap-y-1 px-4 py-3 text-left hover:bg-muted/60 focus-visible:bg-muted focus-visible:outline-none sm:px-5"
                      >
                        <span className="min-w-0 font-medium text-ink">{presence.title}</span>
                        <Badge variant={state.variant}>{state.label}</Badge>
                        <time dateTime={presence.date} className="text-sm text-ink-muted tabular-nums">
                          {formatDateTime(presence.date)}
                        </time>
                        <span className="ml-auto flex items-center gap-3">
                          <AnswerCounts presence={presence} />
                          <ChevronRight className="size-4 text-ink-subtle" aria-hidden />
                        </span>
                      </button>
                    </li>
                  )
                })}
              </ul>
            </Card>
            <Pagination
              page={query.data.page}
              pageSize={query.data.pageSize}
              total={query.data.total}
              onPage={(page) => onSearch({ page: page > 1 ? page : undefined })}
            />
          </>
        )}
      </section>
    </>
  )
}
