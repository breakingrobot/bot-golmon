import { Link } from '@tanstack/react-router'

import type { MemberRef } from '@/api/types'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/misc'
import { cn, initials } from '@/lib/utils'

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

/** Nom d'un membre avec avatar ; lien vers sa fiche quand `guildId` est donné et la fiche visible. */
export function MemberName({
  member,
  guildId,
  linked = true,
  avatar = true,
  className,
}: {
  member: MemberRef
  guildId?: string | undefined
  linked?: boolean
  avatar?: boolean
  className?: string
}) {
  const content = (
    <>
      {avatar && <MemberAvatar member={member} className="size-6" />}
      <span className="truncate">{member.name}</span>
    </>
  )
  const base = cn('inline-flex min-w-0 items-center gap-2 font-medium text-ink', className)
  if (guildId && linked) {
    return (
      <Link
        to="/serveurs/$guildId/membres/$userId"
        params={{ guildId, userId: member.id }}
        className={cn(base, 'rounded-sm hover:text-brand hover:underline')}
      >
        {content}
      </Link>
    )
  }
  return <span className={base}>{content}</span>
}
