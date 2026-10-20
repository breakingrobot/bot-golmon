import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { scheduleUndoable, UNDO_DELAY } from '@/hooks/use-undoable-action'
import type { UndoableAction } from '@/hooks/use-undoable-action'

const toasts = vi.hoisted(() => ({
  show: vi.fn<(message: string, options: { action: { onClick: () => void } }) => void>(),
  error: vi.fn<(message: string) => void>(),
  dismiss: vi.fn<(id: string) => void>(),
}))

vi.mock(import('sonner'), () => ({
  toast: Object.assign(toasts.show, { error: toasts.error, dismiss: toasts.dismiss }),
}))

const action = (key: string, commit: UndoableAction['commit'] = () => Promise.resolve()) => {
  const restore = vi.fn<() => void>()
  const hide = vi.fn<() => () => void>(() => restore)
  const committed = vi.fn<() => void>()
  const send = vi.fn<UndoableAction['commit']>(commit)
  scheduleUndoable({ key, message: 'Cotisation supprimée', hide, commit: send, onCommitted: committed })
  return { hide, restore, send, committed }
}

/** Clic sur « Annuler » du dernier toast affiché. */
const clickUndo = () => toasts.show.mock.lastCall?.[1].action.onClick()

describe('useUndoableAction', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.clearAllMocks()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('retire tout de suite et propose « Annuler »', () => {
    const { hide, send } = action('a')
    expect(hide).toHaveBeenCalledOnce()
    expect(toasts.show).toHaveBeenCalledWith('Cotisation supprimée', expect.objectContaining({ duration: UNDO_DELAY }))
    vi.advanceTimersByTime(UNDO_DELAY - 1)
    expect(send).not.toHaveBeenCalled()
  })

  it('envoie à l’expiration du toast', async () => {
    const { send, committed, restore } = action('a2')
    await vi.advanceTimersByTimeAsync(UNDO_DELAY)
    expect(send).toHaveBeenCalledExactlyOnceWith({ keepalive: false })
    expect(committed).toHaveBeenCalledOnce()
    expect(restore).not.toHaveBeenCalled()
  })

  it('« Annuler » remet l’élément sans rien envoyer', () => {
    const { send, restore } = action('b')
    clickUndo()
    expect(restore).toHaveBeenCalledOnce()
    vi.advanceTimersByTime(UNDO_DELAY * 2)
    expect(send).not.toHaveBeenCalled()
  })

  it('au départ de la page, envoie aussitôt en keepalive', () => {
    const { send } = action('c')
    globalThis.dispatchEvent(new Event('pagehide'))
    expect(toasts.dismiss).toHaveBeenCalledWith('c')
    vi.advanceTimersByTime(UNDO_DELAY)
    expect(send).toHaveBeenCalledExactlyOnceWith({ keepalive: true })
  })

  it('refus du serveur : l’élément revient, avec le message', async () => {
    const { restore, committed } = action('d', () => Promise.reject(new Error('Tu ne peux pas.')))
    await vi.advanceTimersByTimeAsync(UNDO_DELAY)
    expect(restore).toHaveBeenCalledOnce()
    expect(committed).not.toHaveBeenCalled()
    expect(toasts.error).toHaveBeenCalledWith('Tu ne peux pas.')
  })

  it('la même action relancée envoie d’abord la précédente', () => {
    const first = action('e')
    const second = action('e')
    expect(first.send).toHaveBeenCalledOnce()
    expect(second.send).not.toHaveBeenCalled()
    vi.advanceTimersByTime(UNDO_DELAY)
    expect(second.send).toHaveBeenCalledOnce()
  })
})
