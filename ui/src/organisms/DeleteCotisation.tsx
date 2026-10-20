// Suppression d'une cotisation sans boîte de confirmation : elle disparaît aussitôt et « Annuler »
// reste proposé 6 s avant l'envoi (useUndoableAction). La suppression est inscrite au journal du staff.

import { Trash2 } from 'lucide-react'

import { useUndoableDeleteCotisation } from '@/api/queries-site'
import type { CotisationEntry } from '@/api/types'
import { Button } from '@/atoms/Button'
import { formatDay } from '@/lib/format'
import { useGuild } from '@/lib/guild-context'

export type DeleteCotisationProps = { entry: CotisationEntry }

export const DeleteCotisation = ({ entry }: DeleteCotisationProps) => {
  const { guild } = useGuild()
  const remove = useUndoableDeleteCotisation(guild.id)
  return (
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={`Supprimer la cotisation du ${formatDay(entry.date)} de ${entry.member.name}`}
      onClick={() => remove(entry)}
    >
      <Trash2 aria-hidden />
    </Button>
  )
}
