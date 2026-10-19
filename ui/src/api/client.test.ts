import { beforeEach, describe, expect, it, vi } from 'vitest'

import { jsonResponse } from '@/test/render'

// Le client garde un état de module (fetch choisi, redirection en cours) : un module neuf par test.
const loadClient = () => {
  vi.resetModules()
  return import('@/api/client')
}

describe("client de l'API", () => {
  beforeEach(() => {
    globalThis.history.replaceState(null, '', '/serveurs/42/coffres?objet=medikit')
  })

  it('envoie les écritures en JSON avec l’en-tête anti-CSRF', async () => {
    const fetchMock = vi.fn<typeof fetch>(() => jsonResponse({ id: '7' }, 201))
    vi.stubGlobal('fetch', fetchMock)
    const { api } = await loadClient()

    await expect(api('POST', '/guilds/42/contacts', { body: { name: 'Ballas' } })).resolves.toStrictEqual({ id: '7' })

    expect(fetchMock).toHaveBeenCalledWith(
      '/api/guilds/42/contacts',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ name: 'Ballas' }),
        headers: expect.objectContaining({ 'X-Requested-With': 'fetch', 'Content-Type': 'application/json' }),
      }),
    )
  })

  it('omet les paramètres vides de la requête', async () => {
    const fetchMock = vi.fn<typeof fetch>(() => jsonResponse([]))
    vi.stubGlobal('fetch', fetchMock)
    const { api } = await loadClient()

    await api('GET', '/guilds/42/journal', { query: { action: '', page: 2, q: undefined, archived: false } })

    expect(fetchMock).toHaveBeenCalledWith('/api/guilds/42/journal?page=2', expect.anything())
  })

  it('porte le message du serveur dans ApiRequestError', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>(() => jsonResponse({ error: "Kyle n'a pas le rôle membre : rien n'est enregistré." }, 422)),
    )
    const { api, ApiRequestError, errorMessage } = await loadClient()

    const failure = await api('POST', '/guilds/42/cotisations', { body: {} }).catch((error: unknown) => error)

    expect(failure).toBeInstanceOf(ApiRequestError)
    expect(failure).toMatchObject({ status: 422 })
    expect(errorMessage(failure)).toBe("Kyle n'a pas le rôle membre : rien n'est enregistré.")
  })

  it('donne un message français quand le corps est vide ou illisible', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>(() => Promise.resolve(new Response('<html>', { status: 403 }))),
    )
    const { api, errorMessage } = await loadClient()

    const failure = await api('GET', '/guilds/42/permissions').catch((error: unknown) => error)

    expect(errorMessage(failure)).toBe('Accès refusé.')
  })

  it('signale un serveur injoignable', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>(() => {
        throw new TypeError('Failed to fetch')
      }),
    )
    const { api, errorMessage } = await loadClient()

    const failure = await api('GET', '/me').catch((error: unknown) => error)

    expect(errorMessage(failure)).toBe('Le serveur ne répond pas. Vérifiez votre connexion.')
  })

  it('suit loginUrl sur 401 en revenant à la page affichée', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>(() => jsonResponse({ error: 'Connexion requise.', loginUrl: '/login?next=/' }, 401)),
    )
    const assign = vi.spyOn(globalThis.location, 'assign').mockImplementation(() => {
      /* pas de navigation pendant le test */
    })
    const { api } = await loadClient()

    await expect(api('GET', '/guilds/42/coffres/stock')).rejects.toMatchObject({ status: 401 })

    expect(assign).toHaveBeenCalledOnce()
    const target = new URL(String(assign.mock.calls[0]?.[0]), globalThis.location.origin)
    expect(target.pathname).toBe('/login')
    expect(target.searchParams.get('next')).toBe('/serveurs/42/coffres?objet=medikit')
  })

  it('ne redirige qu’une fois pour plusieurs 401 simultanés', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>(() => jsonResponse({ error: 'Connexion requise.', loginUrl: '/login' }, 401)),
    )
    const assign = vi.spyOn(globalThis.location, 'assign').mockImplementation(() => {
      /* pas de navigation pendant le test */
    })
    const { api } = await loadClient()

    await Promise.allSettled([api('GET', '/a'), api('GET', '/b'), api('GET', '/c')])

    expect(assign).toHaveBeenCalledOnce()
  })

  it('ne réécrit pas une adresse de connexion externe', async () => {
    const { loginHref } = await loadClient()
    expect(loginHref('https://discord.com/oauth2/authorize?x=1')).toBe('https://discord.com/oauth2/authorize?x=1')
  })
})
