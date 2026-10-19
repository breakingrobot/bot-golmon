import type { MemberRef } from '@/api/types'
import { Avatar, AvatarFallback, AvatarImage } from '@/atoms/Avatar'
import { initials } from '@/lib/utils'

export function MemberAvatar({
  member,
  className,
}: {
  member: Pick<MemberRef, 'name' | 'avatarUrl'>
  className?: string
}) {
  return (
    <Avatar className={className}>
      {member.avatarUrl && <AvatarImage src={member.avatarUrl} alt="" />}
      <AvatarFallback aria-hidden>{initials(member.name)}</AvatarFallback>
    </Avatar>
  )
}
