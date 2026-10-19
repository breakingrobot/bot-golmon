import { Paperclip } from 'lucide-react'

import type { LogEntry } from '@/api/types'
import { formatTime } from '@/lib/format'
import { cn, initials } from '@/lib/utils'
import { RichHtml } from '@/molecules/RichHtml'

export const TONE: Record<LogEntry['kind'], { card: string; avatar: string; badge: string }> = {
  user: { card: 'border-l-4 border-l-user', avatar: 'bg-user-soft text-user', badge: 'bg-user-soft text-user' },
  staff: { card: 'border-l-4 border-l-staff', avatar: 'bg-staff-soft text-staff', badge: 'bg-staff-soft text-staff' },
  internal: {
    card: 'border-internal/40 bg-internal-soft/40',
    avatar: 'bg-internal-soft text-internal',
    badge: 'bg-internal-soft text-internal',
  },
  command: { card: 'border-dashed', avatar: 'bg-system-soft text-system', badge: 'bg-system-soft text-system' },
  system: { card: 'bg-surface-sunken', avatar: 'bg-system-soft text-system', badge: 'bg-system-soft text-system' },
  danger: { card: 'border-danger/40', avatar: 'bg-danger-soft text-danger', badge: 'bg-danger-soft text-danger' },
}

export type LogEntryCardProps = { entry: LogEntry; verbose: boolean }

export const LogEntryCard = ({ entry, verbose }: LogEntryCardProps) => {
  const tone = TONE[entry.kind]
  return (
    <article className={cn('rounded-xl border bg-card px-4 py-3 sm:px-5', tone.card)}>
      <header className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <span
          className={cn(
            'flex size-7 shrink-0 items-center justify-center rounded-full text-xs font-semibold',
            tone.avatar,
          )}
          aria-hidden
        >
          {initials(entry.author).slice(0, 1)}
        </span>
        <span className="font-semibold text-ink">{entry.author}</span>
        {entry.role && <span className="text-sm text-ink-muted">{entry.role}</span>}
        {entry.badge && (
          <span
            className={cn(
              'rounded-md px-1.5 py-0.5 text-[0.6875rem] font-semibold tracking-wide uppercase',
              tone.badge,
            )}
          >
            {entry.badge}
          </span>
        )}
        {entry.number !== null && <span className="font-mono text-xs text-ink-subtle">#{entry.number}</span>}
        <time
          dateTime={entry.at}
          className="ml-auto font-mono text-xs text-ink-subtle"
          title={new Date(entry.at).toLocaleString('fr-FR')}
        >
          {formatTime(entry.at)}
        </time>
      </header>

      {entry.edit ? (
        <div className="mt-3 grid gap-2 sm:grid-cols-2">
          <div className="rounded-lg bg-danger-soft px-3 py-2">
            <p className="text-[0.6875rem] font-semibold tracking-wide text-danger uppercase">Avant</p>
            <RichHtml html={entry.edit.beforeHtml} className="mt-1 text-ink" />
          </div>
          <div className="rounded-lg bg-staff-soft px-3 py-2">
            <p className="text-[0.6875rem] font-semibold tracking-wide text-staff uppercase">Après</p>
            <RichHtml html={entry.edit.afterHtml} className="mt-1 text-ink" />
          </div>
        </div>
      ) : (
        entry.bodyHtml && (
          <RichHtml
            html={entry.bodyHtml}
            className={cn('mt-2', entry.monospace ? 'font-mono text-sm text-ink-muted' : 'text-ink')}
          />
        )
      )}

      {entry.images.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-2">
          {entry.images.map((image) => (
            <a
              key={image}
              href={image}
              target="_blank"
              rel="noopener noreferrer"
              className="block overflow-hidden rounded-lg border"
            >
              <img
                src={image}
                alt="Pièce jointe"
                loading="lazy"
                className="block max-h-64 min-h-16 max-w-full min-w-24 bg-muted object-contain text-xs text-ink-subtle"
              />
            </a>
          ))}
        </div>
      )}

      {entry.files.length > 0 && (
        <ul className="mt-3 flex flex-wrap gap-2">
          {entry.files.map((file) => (
            <li key={file.url}>
              <a
                href={file.url}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex max-w-full items-center gap-2 rounded-lg border bg-muted px-3 py-1.5 text-sm text-ink-muted hover:border-line-strong hover:text-ink"
              >
                <Paperclip className="size-4" aria-hidden />
                <span className="truncate">{file.name}</span>
              </a>
            </li>
          ))}
        </ul>
      )}

      {entry.embeds && (
        <p className="mt-3 rounded-lg border-l-4 border-line-strong bg-muted px-3 py-2 text-sm text-ink-muted">
          {entry.embeds}
        </p>
      )}

      {verbose && entry.technical && (
        <p className="mt-2 font-mono text-xs break-all text-ink-subtle">{entry.technical}</p>
      )}
    </article>
  )
}
