// En-tête du site, choix du thème, menu du compte.

import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { LogOut, Monitor, Moon, Sun, Ticket } from 'lucide-react'
import type { ReactNode } from 'react'

import { meQuery } from '@/api/queries'
import { LoginLink } from '@/components/login-link'
import { MemberAvatar } from '@/components/member'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useTheme, type ThemeChoice } from '@/lib/theme'

export function ThemeToggle() {
  const { choice, resolved, setChoice } = useTheme()
  const Icon = choice === 'system' ? Monitor : resolved === 'dark' ? Moon : Sun
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" aria-label="Thème de l'affichage">
          <Icon aria-hidden />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuLabel>Thème</DropdownMenuLabel>
        <DropdownMenuRadioGroup value={choice} onValueChange={(value) => setChoice(value as ThemeChoice)}>
          <DropdownMenuRadioItem value="system">
            <Monitor aria-hidden /> Système
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="light">
            <Sun aria-hidden /> Clair
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="dark">
            <Moon aria-hidden /> Sombre
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

export function SiteHeader({ menu }: { menu?: ReactNode }) {
  const me = useQuery(meQuery)
  const user = me.data?.user ?? null
  return (
    <header className="sticky top-0 z-40 border-b bg-card/90 backdrop-blur supports-[backdrop-filter]:bg-card/75">
      <div className="mx-auto flex h-14 max-w-7xl items-center gap-2 px-4 sm:px-6">
        {menu}
        <Link to="/" className="flex items-center gap-2 rounded-md font-semibold tracking-tight text-ink">
          <img src="/favicon.svg" alt="" className="size-6" />
          <span>Bot Discord</span>
        </Link>
        <nav className="ml-2 hidden items-center gap-1 sm:flex" aria-label="Navigation principale">
          {me.data?.tickets && (
            <Link
              to="/tickets"
              className="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium text-ink-muted hover:text-ink"
              activeProps={{ className: 'bg-brand-soft text-brand hover:text-brand', 'aria-current': 'page' }}
            >
              <Ticket className="size-4" aria-hidden /> Tickets
            </Link>
          )}
        </nav>
        <div className="ml-auto flex items-center gap-1">
          <ThemeToggle />
          {user ? (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" className="gap-2 px-2" aria-label={`Compte : ${user.name}`}>
                  <MemberAvatar member={user} className="size-7" />
                  <span className="hidden max-w-40 truncate text-ink sm:inline">{user.name}</span>
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuLabel>{user.name}</DropdownMenuLabel>
                {me.data?.tickets && (
                  <DropdownMenuItem asChild className="sm:hidden">
                    <Link to="/tickets">
                      <Ticket aria-hidden /> Tickets
                    </Link>
                  </DropdownMenuItem>
                )}
                <DropdownMenuSeparator />
                <DropdownMenuItem asChild>
                  {/* Déconnexion servie par le bot : navigation complète. */}
                  <a href="/logout">
                    <LogOut aria-hidden /> Déconnexion
                  </a>
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          ) : (
            me.data?.loginEnabled && (
              <Button asChild size="sm">
                <LoginLink loginUrl={me.data.loginUrl}>Se connecter</LoginLink>
              </Button>
            )
          )}
        </div>
      </div>
    </header>
  )
}

export function PageContainer({ children, wide = false }: { children: ReactNode; wide?: boolean }) {
  return (
    <main
      id="contenu"
      className={wide ? 'mx-auto w-full max-w-7xl px-4 py-6 sm:px-6' : 'mx-auto w-full max-w-5xl px-4 py-8 sm:px-6'}
    >
      {children}
    </main>
  )
}
