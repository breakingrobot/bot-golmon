import { Outlet } from '@tanstack/react-router'
import { useAtom, useSetAtom } from 'jotai'
import { Menu } from 'lucide-react'
import { useEffect } from 'react'

import type { GuildAccess, Me } from '@/api/types'
import { Button } from '@/atoms/Button'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle, SheetTrigger } from '@/atoms/Sheet'
import { GuildContext } from '@/lib/guild-context'
import { GuildNav } from '@/organisms/GuildNav'
import { guildNavOpenAtom } from '@/state/guild-nav'
import { lastGuildIdAtom } from '@/state/last-guild'

export function GuildShell({ me, guild, userId }: { me: Me; guild: GuildAccess; userId: string }) {
  const [open, setOpen] = useAtom(guildNavOpenAtom)
  const setLastGuildId = useSetAtom(lastGuildIdAtom)
  const context = { me, guild, userId }
  useEffect(() => {
    setLastGuildId(guild.id)
  }, [guild.id, setLastGuildId])
  // Tiroir refermé en quittant l'espace du serveur (l'atome survit au démontage).
  useEffect(() => () => setOpen(false), [setOpen])
  return (
    <GuildContext value={context}>
      <div className="mx-auto flex w-full max-w-7xl flex-1">
        <aside className="hidden w-60 shrink-0 border-r lg:block">
          <div className="sticky top-14 max-h-[calc(100dvh-3.5rem)] overflow-y-auto px-3 py-5">
            <GuildNav me={me} guild={guild} userId={userId} />
          </div>
        </aside>
        <div className="min-w-0 flex-1">
          <div className="sticky top-14 z-30 flex items-center gap-2 border-b bg-canvas/95 px-4 py-2 backdrop-blur lg:hidden">
            <Sheet open={open} onOpenChange={setOpen}>
              <SheetTrigger asChild>
                <Button variant="outline" size="sm" aria-label="Ouvrir le menu du serveur">
                  <Menu aria-hidden /> Menu
                </Button>
              </SheetTrigger>
              <SheetContent side="left" className="w-72 overflow-y-auto">
                <SheetHeader>
                  <SheetTitle>{guild.name}</SheetTitle>
                  <SheetDescription className="sr-only">Pages du serveur</SheetDescription>
                </SheetHeader>
                <div className="px-3 pb-6">
                  <GuildNav me={me} guild={guild} userId={userId} onNavigate={() => setOpen(false)} />
                </div>
              </SheetContent>
            </Sheet>
            <span className="truncate text-sm font-medium text-ink">{guild.name}</span>
          </div>
          <main id="contenu" className="px-4 py-6 sm:px-6 lg:px-8">
            <Outlet />
          </main>
        </div>
      </div>
    </GuildContext>
  )
}
