import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { ContactsView } from '@/pages/contacts/ContactsView'

export const ContactsPage = () => {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'contact.voir')) return <Forbidden />
  return <ContactsView />
}
