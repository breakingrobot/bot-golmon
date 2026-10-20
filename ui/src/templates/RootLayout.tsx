import { Outlet } from '@tanstack/react-router'

import { SiteHeader } from '@/organisms/SiteHeader'

export const RootLayout = () => (
  <div className="flex min-h-dvh flex-col">
    <a
      href="#contenu"
      className="sr-only z-50 rounded-md bg-card px-3 py-2 focus:not-sr-only focus:fixed focus:top-2 focus:left-2"
    >
      Aller au contenu
    </a>
    <SiteHeader />
    <Outlet />
  </div>
)
