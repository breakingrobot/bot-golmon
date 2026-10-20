// Palette de commandes et accueil, dans l'application complète servie par la maquette de l'API.

import { QueryClientProvider } from '@tanstack/react-query'
import { createMemoryHistory, RouterProvider } from '@tanstack/react-router'
import { render, screen, waitFor } from '@testing-library/react'
import { userEvent } from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { createAppRouter } from '@/router'
import { createTestQueryClient } from '@/test/render'

const GUILD = '1187000000000000042'

const renderApp = (path: string) => {
  const router = createAppRouter(createTestQueryClient(), createMemoryHistory({ initialEntries: [path] }))
  render(
    <QueryClientProvider client={router.options.context.queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  )
  return router
}

describe('accueil et palette de commandes (maquette)', () => {
  beforeEach(() => {
    vi.stubEnv('VITE_MOCK', '1')
  })

  it('l’accueil montre les tuiles ouvertes au compte', async () => {
    renderApp(`/serveurs/${GUILD}`)
    expect(await screen.findByRole('heading', { level: 1, name: 'Accueil' }, { timeout: 5000 })).toBeVisible()
    expect(await screen.findByRole('heading', { level: 2, name: 'Objectif de la semaine' })).toBeVisible()
    expect(screen.getByRole('link', { name: /Stock bas/u })).toHaveAttribute('href', `/serveurs/${GUILD}/coffres`)
  })

  it('Ctrl+K ouvre la palette ; flèches et Entrée naviguent', async () => {
    const user = userEvent.setup()
    const router = renderApp(`/serveurs/${GUILD}`)
    await screen.findByRole('heading', { level: 1, name: 'Accueil' }, { timeout: 5000 })

    await user.keyboard('{Control>}k{/Control}')
    const input = await screen.findByRole('combobox', { name: /Rechercher/u })
    expect(input).toHaveFocus()
    await user.type(input, 'journal staff')
    const option = await screen.findByRole('option', { name: /Journal du staff/u })
    expect(option).toHaveAttribute('aria-selected', 'true')
    expect(input).toHaveAttribute('aria-activedescendant', option.id)

    await user.keyboard('{Enter}')
    await waitFor(() => expect(router.state.location.pathname).toBe(`/serveurs/${GUILD}/journal`))
    expect(screen.queryByRole('combobox', { name: /Rechercher/u })).not.toBeInTheDocument()
  })

  it('Échap referme la palette', async () => {
    const user = userEvent.setup()
    renderApp(`/serveurs/${GUILD}`)
    await screen.findByRole('heading', { level: 1, name: 'Accueil' }, { timeout: 5000 })
    await user.click(screen.getByRole('button', { name: 'Rechercher (Ctrl+K)' }))
    expect(await screen.findByRole('combobox', { name: /Rechercher/u })).toBeVisible()
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('combobox', { name: /Rechercher/u })).not.toBeInTheDocument())
  })
})
