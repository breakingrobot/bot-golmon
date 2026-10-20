// Mise à jour en direct d'un serveur : flux SSE `/api/guilds/:id/events` (EventSource, même origine,
// cookie de session). Chaque sujet reçu invalide ses requêtes (src/lib/live.ts), regroupées sur
// 300 ms pour qu'une rafale de mouvements de coffre ne recharge qu'une fois. Coupure : reconnexion
// avec une attente croissante ; à la reprise, tout le serveur est rechargé (changements manqués).

import { useQueryClient } from '@tanstack/react-query'
import type { QueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'

import { isLiveMessage, queryKeysFor, reconnectDelay } from '@/lib/live'
import type { LiveMessage } from '@/lib/live'

export type LiveStatus = 'connecting' | 'live' | 'retrying' | 'unavailable'

/** Regroupement des sujets reçus avant invalidation. */
const FLUSH_DELAY = 300

const supported = () => import.meta.env.VITE_MOCK !== '1' && typeof EventSource !== 'undefined'

/** Ouvre le flux du serveur ; renvoie la fermeture (reconnexions comprises). */
const follow = (guildId: string, client: QueryClient, setStatus: (status: LiveStatus) => void) => {
  let source: EventSource | null = null
  let retry: ReturnType<typeof setTimeout> | undefined
  let flush: ReturnType<typeof setTimeout> | undefined
  let attempt = 0
  let stopped = false
  const pending = new Set<LiveMessage>()

  const invalidate = () => {
    flush = undefined
    const messages = pending.has('*') ? (['*'] as const) : [...pending]
    pending.clear()
    for (const message of messages) {
      for (const queryKey of queryKeysFor(guildId, message)) void client.invalidateQueries({ queryKey })
    }
  }
  const receive = (message: LiveMessage) => {
    pending.add(message)
    flush ??= globalThis.setTimeout(invalidate, FLUSH_DELAY)
  }

  const connect = () => {
    const current = new EventSource(`/api/guilds/${encodeURIComponent(guildId)}/events`)
    source = current
    current.addEventListener('open', () => {
      if (attempt > 0) receive('*')
      attempt = 0
      setStatus('live')
    })
    current.addEventListener('change', (event) => {
      if (isLiveMessage(event.data)) receive(event.data)
    })
    current.addEventListener('error', () => {
      // Reconnexion gérée ici (attente croissante), pas par le navigateur.
      current.close()
      if (stopped) return
      setStatus('retrying')
      retry = globalThis.setTimeout(connect, reconnectDelay(attempt))
      attempt += 1
    })
  }

  connect()
  return () => {
    stopped = true
    globalThis.clearTimeout(retry)
    globalThis.clearTimeout(flush)
    source?.close()
  }
}

export const useLiveUpdates = (guildId: string): LiveStatus => {
  const client = useQueryClient()
  const [status, setStatus] = useState<LiveStatus>(() => (supported() ? 'connecting' : 'unavailable'))
  useEffect(() => (supported() ? follow(guildId, client, setStatus) : undefined), [client, guildId])
  return status
}
