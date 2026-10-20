import { useQuery } from '@tanstack/react-query'
import { X } from 'lucide-react'

import { memberProfileQuery } from '@/api/queries'
import type { MemberRef } from '@/api/types'
import { Button } from '@/atoms/Button'
import { MemberPicker } from '@/molecules/MemberPicker'

export type MemberFilterProps = {
  guildId: string
  memberId: string | undefined
  known: MemberRef[]
  onChange: (memberId?: string) => void
}

export const MemberFilter = ({ guildId, memberId, known, onChange }: MemberFilterProps) => {
  // Nom du membre filtré : pris dans le classement, sinon dans sa fiche.
  const fromList = known.find((item) => item.id === memberId)
  const profile = useQuery({ ...memberProfileQuery(guildId, memberId ?? ''), enabled: Boolean(memberId) && !fromList })
  const current = fromList ?? profile.data?.member ?? null
  return (
    <div className="flex min-w-56 items-center gap-1">
      <div className="min-w-0 flex-1">
        <MemberPicker
          guildId={guildId}
          value={current}
          onChange={(member) => onChange(member?.id)}
          placeholder="Tous les membres"
        />
      </div>
      {memberId && (
        <Button variant="ghost" size="icon-sm" onClick={() => onChange()} aria-label="Voir tous les membres">
          <X aria-hidden />
        </Button>
      )}
    </div>
  )
}
