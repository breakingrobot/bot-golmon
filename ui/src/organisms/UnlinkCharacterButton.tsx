// Délier un personnage : effet immédiat, « Annuler » proposé 6 s avant l'envoi (useUndoableAction).
// L'historique des mouvements est conservé.

import { Unlink } from 'lucide-react'

import { useUndoableUnlinkPlayer } from '@/api/queries-site'
import type { MemberRef } from '@/api/types'
import { Button } from '@/atoms/Button'

export type UnlinkCharacterButtonProps = {
  guildId: string
  gameId: string
  name: string
  member: MemberRef
}

export const UnlinkCharacterButton = ({ guildId, gameId, name, member }: UnlinkCharacterButtonProps) => {
  const unlink = useUndoableUnlinkPlayer(guildId)
  return (
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={`Délier ${name} de ${member.name}`}
      onClick={() => unlink({ gameId, name })}
    >
      <Unlink aria-hidden />
    </Button>
  )
}
