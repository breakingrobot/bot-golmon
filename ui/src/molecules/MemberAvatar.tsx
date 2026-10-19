import type { MemberRef } from '@/api/types'
import { Avatar, AvatarFallback, AvatarImage } from '@/atoms/Avatar'
import { initials } from '@/lib/utils'

export type MemberAvatarProps = {
  member: Pick<MemberRef, 'name' | 'avatarUrl'>
  className?: string
}

export const MemberAvatar = ({ member, className }: MemberAvatarProps) => (
  <Avatar className={className}>
    {member.avatarUrl && <AvatarImage src={member.avatarUrl} alt="" />}
    <AvatarFallback aria-hidden>{initials(member.name)}</AvatarFallback>
  </Avatar>
)
