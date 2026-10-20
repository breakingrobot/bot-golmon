import { render, screen, waitFor } from '@testing-library/react'
import { userEvent } from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { CsvExportButton } from '@/molecules/CsvExportButton'

const rows = [{ name: 'Les Ballas', notes: 2 }]
const columns = [
  { header: 'Nom', value: (row: (typeof rows)[number]) => row.name },
  { header: 'Notes', value: (row: (typeof rows)[number]) => row.notes },
]

/** Contenu du dernier fichier proposé au téléchargement. */
const captureDownload = () => {
  const blobs: Blob[] = []
  vi.spyOn(URL, 'createObjectURL').mockImplementation((blob) => {
    blobs.push(blob as Blob)
    return 'blob:test'
  })
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {
    // Rien à libérer en test.
  })
  const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {
    // Pas de navigation en test.
  })
  return { blobs, click }
}

describe('CsvExportButton', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('télécharge les lignes affichées au format des exports du bot', async () => {
    const { blobs, click } = captureDownload()
    render(<CsvExportButton filename="contacts" columns={columns} rows={rows} />)
    await userEvent.setup().click(screen.getByRole('button', { name: 'Exporter en CSV' }))
    expect(click).toHaveBeenCalledOnce()
    expect(await blobs[0]?.text()).toBe('﻿Nom;Notes\r\nLes Ballas;2\r\n')
  })

  it('charge toutes les pages au clic quand `rows` est une fonction', async () => {
    const { blobs } = captureDownload()
    const load = vi.fn<() => Promise<typeof rows>>(() => Promise.resolve(rows))
    render(<CsvExportButton filename="journal" columns={columns} rows={load} />)
    await userEvent.setup().click(screen.getByRole('button', { name: 'Exporter en CSV' }))
    await waitFor(() => expect(blobs).toHaveLength(1))
    expect(load).toHaveBeenCalledOnce()
  })
})
