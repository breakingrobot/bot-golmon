import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { JournalView } from '@/pages/journal/JournalView'

export function JournalPage() {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'site.journal')) return <Forbidden />
  return <JournalView />
}
