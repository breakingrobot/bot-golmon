import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowRight, ScrollText } from 'lucide-react'

import { journalQuery } from '@/api/queries'
import { Field } from '@/components/form'
import { MemberName } from '@/components/member'
import { EmptyState, Forbidden, PageHeader, Pagination, QueryError, TableSkeleton } from '@/components/page'
import { RichHtml } from '@/components/rich-html'
import { Card } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { formatDateTime, formatNumber } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { ALL, compact, fromSelect, searchPage, searchString } from '@/lib/search'
import { cn } from '@/lib/utils'

type Search = { action?: string; page?: number }

export const Route = createFileRoute('/serveurs/$guildId/journal')({
  validateSearch: (search: Record<string, unknown>): Search =>
    compact({ action: searchString(search.action), page: searchPage(search.page) }),
  component: JournalPage,
})

function JournalPage() {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'site.journal')) return <Forbidden />
  return <Journal />
}

function Journal() {
  const { guild } = useGuild()
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const query = useQuery(journalQuery(guild.id, search.action, search.page ?? 1))
  const linkProfiles = hasPerm(guild, 'membre.fiche') ? guild.id : undefined

  return (
    <>
      <PageHeader
        title="Journal du staff"
        description={query.data ? `${guild.name} · ${formatNumber(query.data.list.total)} action(s)` : guild.name}
        actions={
          <Field label="Action" className="w-64 max-w-full">
            {(props) => (
              <Select
                value={search.action ?? ALL}
                onValueChange={(value) => void navigate({ search: compact({ action: fromSelect(value) }) })}
              >
                <SelectTrigger {...props}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={ALL}>Toutes les actions</SelectItem>
                  {query.data?.actions.map((action) => (
                    <SelectItem key={action.key} value={action.key}>
                      {action.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </Field>
        }
      />
      {query.isPending ? (
        <TableSkeleton rows={8} />
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : query.data.list.items.length === 0 ? (
        <EmptyState title="Aucune action enregistrée" icon={<ScrollText />} />
      ) : (
        <>
          <Card className={cn('gap-0 py-0', query.isPlaceholderData && 'opacity-60')}>
            <ol className="divide-y">
              {query.data.list.items.map((entry) => (
                <li key={entry.id} className="px-5 py-3">
                  <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
                    <span className="font-medium text-ink">{entry.label}</span>
                    {entry.target && (
                      <span className="inline-flex items-center gap-1.5 text-sm text-ink-muted">
                        <ArrowRight className="size-3.5" aria-hidden />
                        <MemberName
                          member={entry.target}
                          guildId={linkProfiles}
                          avatar={false}
                          className="font-normal text-ink-muted"
                        />
                      </span>
                    )}
                    <time dateTime={entry.at} className="ml-auto text-sm text-ink-subtle tabular-nums">
                      {formatDateTime(entry.at)}
                    </time>
                  </div>
                  <p className="mt-0.5 flex items-center gap-1 text-sm text-ink-muted">
                    par <MemberName member={entry.actor} guildId={linkProfiles} avatar={false} />
                  </p>
                  {entry.detailsHtml && <RichHtml html={entry.detailsHtml} className="mt-1 text-sm text-ink" />}
                </li>
              ))}
            </ol>
          </Card>
          <Pagination
            page={query.data.list.page}
            pageSize={query.data.list.pageSize}
            total={query.data.list.total}
            onPage={(page) =>
              void navigate({ search: (prev) => compact({ ...prev, page: page > 1 ? page : undefined }) })
            }
            newerLabel="Plus récentes"
            olderLabel="Plus anciennes"
          />
        </>
      )}
    </>
  )
}
