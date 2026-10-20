import { useQuery } from '@tanstack/react-query'
import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { ArrowRight, ScrollText } from 'lucide-react'

import { journalQuery } from '@/api/queries'
import { fetchJournalForExport } from '@/api/queries-site'
import type { JournalEntry } from '@/api/types'
import { Card } from '@/atoms/Card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { csvDateTime, htmlToText } from '@/lib/csv'
import type { CsvColumn } from '@/lib/csv'
import { formatDateTime, formatNumber } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { ALL, compact, fromSelect } from '@/lib/search'
import { cn } from '@/lib/utils'
import { CsvExportButton } from '@/molecules/CsvExportButton'
import { EmptyState } from '@/molecules/EmptyState'
import { Field } from '@/molecules/Field'
import { MemberName } from '@/molecules/MemberName'
import { PageHeader } from '@/molecules/PageHeader'
import { Pagination } from '@/molecules/Pagination'
import { QueryError } from '@/molecules/QueryError'
import { RichHtml } from '@/molecules/RichHtml'
import { TableSkeleton } from '@/molecules/TableSkeleton'

const route = getRouteApi('/serveurs/$guildId/journal')

const JOURNAL_CSV: CsvColumn<JournalEntry>[] = [
  { header: 'Date', value: (entry) => csvDateTime(entry.at) },
  { header: 'Action', value: (entry) => entry.label },
  { header: 'Par', value: (entry) => entry.actor.name },
  { header: 'Membre visé', value: (entry) => entry.target?.name },
  { header: 'Détails', value: (entry) => htmlToText(entry.detailsHtml) },
]

export const JournalView = () => {
  const { guild } = useGuild()
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/serveurs/$guildId/journal' })
  const query = useQuery(journalQuery(guild.id, search.action, search.page ?? 1))
  const linkProfiles = hasPerm(guild, 'membre.fiche') ? guild.id : undefined

  return (
    <>
      <PageHeader
        title="Journal du staff"
        description={query.data ? `${guild.name} · ${formatNumber(query.data.list.total)} action(s)` : guild.name}
        actions={
          <>
            <CsvExportButton
              filename="journal"
              columns={JOURNAL_CSV}
              rows={() => fetchJournalForExport(guild.id, search.action)}
              disabled={!query.data?.list.total}
            />
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
          </>
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
              void navigate({
                search: (prev) => compact({ ...prev, page: page > 1 ? page : undefined }),
              })
            }
            newerLabel="Plus récentes"
            olderLabel="Plus anciennes"
          />
        </>
      )}
    </>
  )
}
