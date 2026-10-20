import { screen, waitFor, within } from '@testing-library/react'
import { userEvent } from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import type { CoffreStock } from '@/api/types'
import { Button } from '@/atoms/Button'
import { CoffreInventoryDialog } from '@/organisms/CoffreInventoryDialog'
import { jsonResponse, renderInGuild, TEST_GUILD } from '@/test/render'

const PROPERTY = { id: '3307', name: 'Garage Vespucci' }

const STOCK: CoffreStock = {
  properties: [
    {
      ...PROPERTY,
      items: [
        {
          item: 'repair_kit',
          label: 'Kit de réparation',
          deposited: 0,
          withdrawn: 14,
          adjusted: 0,
          quantity: -14,
          low: true,
        },
        { item: 'lockpick', label: 'Crochet', deposited: 4, withdrawn: 2, adjusted: 0, quantity: 2, low: false },
        {
          item: 'medikit',
          label: 'Trousse de soins',
          deposited: 1,
          withdrawn: 3,
          adjusted: 0,
          quantity: -2,
          low: true,
        },
      ],
    },
  ],
}

/** Faux serveur : stock en GET, réponse donnée pour le PUT de l'inventaire. */
const serve = (put: () => Promise<Response>) => {
  const fetchMock = vi.fn<typeof fetch>((input, init) =>
    init?.method === 'PUT' ? put() : jsonResponse(String(input).includes('/coffres/stock') ? STOCK : {}),
  )
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

const openDialog = async () => {
  const user = userEvent.setup()
  renderInGuild(
    <CoffreInventoryDialog
      guildId={TEST_GUILD.id}
      property={PROPERTY}
      properties={[PROPERTY]}
      items={[{ item: 'ammo_9mm', label: 'Munitions 9 mm' }]}
      trigger={<Button>Inventaire</Button>}
    />,
  )
  await user.click(screen.getByRole('button', { name: 'Inventaire' }))
  await screen.findByLabelText(/Kit de réparation/u)
  return user
}

const putCalls = (fetchMock: ReturnType<typeof serve>) =>
  fetchMock.mock.calls.filter(([, init]) => init?.method === 'PUT')

describe('CoffreInventoryDialog', () => {
  it('prévisualise les écarts et n’envoie que les lignes qui changent', async () => {
    const fetchMock = serve(() =>
      jsonResponse({ corrections: [{ item: 'repair_kit', label: 'Kit de réparation', before: -14, after: 20 }] }),
    )
    const user = await openDialog()

    await user.type(screen.getByLabelText(/Kit de réparation/u), '20')
    // Quantité égale au stock connu : rien à corriger ; la trousse reste vide : non envoyée.
    await user.type(screen.getByLabelText(/Crochet/u), '2')
    await user.type(screen.getByRole('combobox', { name: 'Objet' }), 'ammo')
    await user.click(screen.getByRole('option', { name: /Munitions 9 mm/u }))
    await user.type(screen.getByLabelText('Quantité'), '120')
    await user.click(screen.getByRole('button', { name: 'Ajouter l’objet' }))

    const preview = screen.getByRole('region', { name: 'Aperçu des corrections' })
    expect(within(preview).getByText('2 correction(s) à enregistrer, 1 objet(s) déjà juste(s).')).toBeVisible()
    expect(within(preview).getByText('+34')).toBeVisible()
    expect(within(preview).getByText('+120')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Enregistrer 2 correction(s)' }))

    await waitFor(() => expect(putCalls(fetchMock)).toHaveLength(1))
    const [url, init] = putCalls(fetchMock)[0] ?? []
    expect(url).toBe(`/api/guilds/${TEST_GUILD.id}/coffres/inventory`)
    expect(JSON.parse(String(init?.body))).toStrictEqual({
      propertyId: '3307',
      items: [
        { item: 'repair_kit', quantity: 20 },
        { item: 'ammo_9mm', quantity: 120 },
      ],
    })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
  })

  it('remplit les lignes depuis une liste collée', async () => {
    serve(() => jsonResponse({ corrections: [] }))
    const user = await openDialog()

    await user.click(screen.getByRole('radio', { name: 'Coller une liste' }))
    await user.type(screen.getByLabelText('Liste objet:quantité'), 'medikit:5, Repair Kit 0; acetone=7')
    await user.click(screen.getByRole('button', { name: 'Remplir les lignes' }))

    expect(screen.getByLabelText(/Trousse de soins/u)).toHaveValue('5')
    expect(screen.getByLabelText(/Kit de réparation/u)).toHaveValue('0')
    expect(screen.getByRole('textbox', { name: /Acetone/u })).toHaveValue('7')
    expect(screen.getByRole('button', { name: 'Enregistrer 3 correction(s)' })).toBeVisible()
  })

  it('signale une liste illisible sans toucher aux lignes', async () => {
    serve(() => jsonResponse({ corrections: [] }))
    const user = await openDialog()

    await user.click(screen.getByRole('radio', { name: 'Coller une liste' }))
    await user.type(screen.getByLabelText('Liste objet:quantité'), 'medikit')
    await user.click(screen.getByRole('button', { name: 'Remplir les lignes' }))

    expect(await screen.findByText('« medikit » : écris objet:quantité, par exemple repair_kit:42.')).toBeVisible()
  })

  it('affiche dans le dialogue l’erreur 422 du serveur et le garde ouvert', async () => {
    const fetchMock = serve(() => jsonResponse({ error: 'Entre 1 et 300 objets par inventaire.' }, 422))
    const user = await openDialog()

    await user.type(screen.getByLabelText(/Crochet/u), '9')
    await user.click(screen.getByRole('button', { name: 'Enregistrer 1 correction(s)' }))

    expect(await within(screen.getByRole('dialog')).findByText('Entre 1 et 300 objets par inventaire.')).toBeVisible()
    expect(putCalls(fetchMock)).toHaveLength(1)
    expect(screen.getByRole('dialog')).toBeVisible()
  })

  it('refuse une quantité illisible et un inventaire sans écart, sans appeler l’API', async () => {
    const fetchMock = serve(() => jsonResponse({ corrections: [] }))
    const user = await openDialog()

    await user.click(screen.getByRole('button', { name: 'Enregistrer' }))
    expect(await screen.findByText('Aucun écart avec le stock connu : rien à corriger.')).toBeVisible()

    await user.type(screen.getByLabelText(/Crochet/u), '-4')
    expect(screen.getByLabelText(/Crochet/u)).toHaveAttribute('aria-invalid', 'true')
    await user.click(screen.getByRole('button', { name: 'Enregistrer' }))
    expect(await screen.findByText('Corrigez les quantités signalées.')).toBeVisible()
    expect(putCalls(fetchMock)).toHaveLength(0)
  })
})
