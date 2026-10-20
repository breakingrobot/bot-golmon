// « Annuler » juste après une action : l'élément disparaît tout de suite (cache TanStack Query
// modifié par `hide`), un toast « Supprimé — Annuler » reste quelques secondes, et la requête n'est
// envoyée qu'à son expiration. « Annuler » remet l'élément sans rien envoyer. Onglet fermé ou
// rechargé (`pagehide`) : les actions en attente partent aussitôt, en `keepalive` (la requête
// survit à la fermeture). Refus du serveur : l'élément revient, avec le message en toast.
//
// Les actions en attente sont gardées hors de React : la ligne supprimée se démonte aussitôt
// et une navigation dans le site ne doit ni annuler ni précipiter l'envoi.

import { toast } from 'sonner'

import { errorMessage } from '@/api/client'

/** Durée pendant laquelle « Annuler » est proposé. */
export const UNDO_DELAY = 6000

export type UndoableAction = {
  /** Identifiant de l'élément : relancer la même action envoie d'abord la précédente. */
  key: string
  /** Texte du toast (« Cotisation supprimée »). */
  message: string
  /** Retire l'élément de l'affichage ; renvoie de quoi le remettre. */
  hide: () => () => void
  /** Requête réelle ; `keepalive` quand la page se ferme. */
  commit: (options: { keepalive: boolean }) => Promise<unknown>
  /** Après l'envoi réussi (rechargement des données). */
  onCommitted?: () => void
}

type Pending = { action: UndoableAction; restore: () => void; timer: ReturnType<typeof setTimeout> }

const pending = new Map<string, Pending>()

/** Envoie l'action en attente (expiration, départ de la page, ou action relancée). */
const send = async (key: string, keepalive: boolean) => {
  const entry = pending.get(key)
  if (!entry) return
  pending.delete(key)
  globalThis.clearTimeout(entry.timer)
  if (keepalive) toast.dismiss(key)
  try {
    await entry.action.commit({ keepalive })
  } catch (error) {
    entry.restore()
    toast.error(errorMessage(error))
    return
  }
  entry.action.onCommitted?.()
}

/** « Annuler » : remet l'élément, rien n'est envoyé. */
const undo = (key: string) => {
  const entry = pending.get(key)
  if (!entry) return
  pending.delete(key)
  globalThis.clearTimeout(entry.timer)
  entry.restore()
}

/** Tout ce qui attend part tout de suite (fermeture de l'onglet). */
export const flushUndoable = () => {
  for (const key of pending.keys()) void send(key, true)
}

let listening = false

export const scheduleUndoable = (action: UndoableAction, delay = UNDO_DELAY) => {
  if (!listening) {
    globalThis.addEventListener('pagehide', flushUndoable)
    listening = true
  }
  void send(action.key, false)
  const restore = action.hide()
  const timer = globalThis.setTimeout(() => void send(action.key, false), delay)
  pending.set(action.key, { action, restore, timer })
  toast(action.message, {
    id: action.key,
    duration: delay,
    action: { label: 'Annuler', onClick: () => undo(action.key) },
  })
}

/** Déclencheur d'actions annulables (voir `scheduleUndoable`). */
export const useUndoableAction =
  (delay = UNDO_DELAY) =>
  (action: UndoableAction) =>
    scheduleUndoable(action, delay)
