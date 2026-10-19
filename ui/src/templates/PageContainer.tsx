import type { ReactNode } from 'react'

export type PageContainerProps = { children: ReactNode; wide?: boolean }

export const PageContainer = ({ children, wide = false }: PageContainerProps) => (
  <main
    id="contenu"
    className={wide ? 'mx-auto w-full max-w-7xl px-4 py-6 sm:px-6' : 'mx-auto w-full max-w-5xl px-4 py-8 sm:px-6'}
  >
    {children}
  </main>
)
