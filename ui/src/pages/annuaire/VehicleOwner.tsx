import type { VehicleEntry } from '@/api/types'
import { MemberName } from '@/molecules/MemberName'
import { RoleDot } from '@/molecules/RoleDot'

export type VehicleOwnerProps = { vehicle: VehicleEntry; guildId: string | undefined }

/** Propriétaire Discord d'un véhicule : un membre, ou un rôle. */
export const VehicleOwner = ({ vehicle, guildId }: VehicleOwnerProps) => {
  if (vehicle.ownerMember) return <MemberName member={vehicle.ownerMember} guildId={guildId} />
  if (vehicle.ownerRole) {
    return (
      <span className="inline-flex min-w-0 items-center gap-2 font-medium text-ink">
        <RoleDot color={vehicle.ownerRole.color} />
        <span className="truncate">@{vehicle.ownerRole.name}</span>
      </span>
    )
  }
  return <span className="text-ink-subtle">Inconnu</span>
}
