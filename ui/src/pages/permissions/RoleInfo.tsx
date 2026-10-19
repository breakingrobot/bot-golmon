import type { PermissionsResponse } from '@/api/types'
import { RoleDot } from '@/molecules/RoleDot'

export type RoleInfoProps = { label: string; role: PermissionsResponse['roles']['grade'] }

export const RoleInfo = ({ label, role }: RoleInfoProps) => (
  <div className="grid gap-1.5">
    <span className="text-sm font-medium text-ink-muted">{label}</span>
    <span className="flex h-9 items-center gap-2 text-sm">
      {role ? (
        <>
          <RoleDot color={role.color} />
          {role.name}
        </>
      ) : (
        <span className="text-ink-subtle">Non configuré</span>
      )}
    </span>
  </div>
)
