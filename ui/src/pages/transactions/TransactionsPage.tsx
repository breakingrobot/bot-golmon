import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { TransactionsView } from '@/pages/transactions/TransactionsView'

export function TransactionsPage() {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'transaction.historique')) return <Forbidden />
  return <TransactionsView />
}
