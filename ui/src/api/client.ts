// Client HTTP de l'API du bot (même origine, cookie de session).
//
// - Écritures : `Content-Type: application/json` + `X-Requested-With: fetch` (CSRF).
// - 401 : redirection vers `loginUrl` (connexion Discord gérée par le serveur).
// - Autres erreurs : `ApiRequestError` porte le message `error` du serveur, affichable tel quel.
//
// En mode maquette (`VITE_MOCK=1`), les requêtes sont servies par `src/mocks` ; la
// condition est remplacée à la compilation, le code de maquette n'entre pas dans le build.

import type { ApiError } from './types'

export class ApiRequestError extends Error {
  readonly status: number
  readonly loginUrl: string | null

  constructor(status: number, message: string, loginUrl: string | null = null) {
    super(message)
    this.name = 'ApiRequestError'
    this.status = status
    this.loginUrl = loginUrl
  }
}

type Fetcher = (input: string, init: RequestInit) => Promise<Response>

let fetcher: Promise<Fetcher> | null = null

function getFetcher(): Promise<Fetcher> {
  if (!fetcher) {
    fetcher =
      import.meta.env.VITE_MOCK === '1'
        ? import('../mocks/server').then((module) => module.mockFetch)
        : Promise.resolve((input: string, init: RequestInit) => fetch(input, init))
  }
  return fetcher
}

function isApiError(value: unknown): value is ApiError {
  return typeof value === 'object' && value !== null && typeof (value as { error?: unknown }).error === 'string'
}

const FALLBACK_MESSAGES: Record<number, string> = {
  400: 'Saisie refusée.',
  401: 'Connexion requise.',
  403: 'Accès refusé.',
  404: 'Introuvable.',
  422: 'Saisie refusée.',
  429: 'Trop de requêtes, réessayez dans un instant.',
}

let redirecting = false

/**
 * Adresse de connexion qui ramène à la page courante : le serveur donne `/login?next=/`,
 * `next` est remplacé par l'adresse affichée (chemin + paramètres, même origine).
 */
export function loginHref(loginUrl: string): string {
  try {
    const url = new URL(loginUrl, window.location.origin)
    if (url.origin !== window.location.origin) return loginUrl
    url.searchParams.set('next', `${window.location.pathname}${window.location.search}`)
    return `${url.pathname}${url.search}`
  } catch {
    return loginUrl
  }
}

export type Query = Record<string, string | number | boolean | null | undefined>

/** Chaîne de requête sans les valeurs vides. */
export function toQueryString(query: Query | undefined): string {
  if (!query) return ''
  const params = new URLSearchParams()
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined || value === null || value === '' || value === false) continue
    params.set(key, String(value))
  }
  const text = params.toString()
  return text ? `?${text}` : ''
}

export async function api<T>(
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE',
  path: string,
  options: { query?: Query; body?: unknown; signal?: AbortSignal } = {},
): Promise<T> {
  const doFetch = await getFetcher()
  const headers: Record<string, string> = { Accept: 'application/json' }
  const init: RequestInit = { method, headers, credentials: 'same-origin' }
  if (options.signal) init.signal = options.signal
  if (method !== 'GET') {
    headers['X-Requested-With'] = 'fetch'
    headers['Content-Type'] = 'application/json'
    if (options.body !== undefined) init.body = JSON.stringify(options.body)
  }

  let response: Response
  try {
    response = await doFetch(`/api${path}${toQueryString(options.query)}`, init)
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') throw error
    throw new ApiRequestError(0, 'Le serveur ne répond pas. Vérifiez votre connexion.')
  }

  if (response.status === 204) return undefined as T

  let payload: unknown = null
  const text = await response.text()
  if (text) {
    try {
      payload = JSON.parse(text)
    } catch {
      payload = null
    }
  }

  if (!response.ok) {
    const message = isApiError(payload)
      ? payload.error
      : (FALLBACK_MESSAGES[response.status] ?? `Erreur du serveur (${response.status}).`)
    const loginUrl = isApiError(payload) ? (payload.loginUrl ?? null) : null
    if (response.status === 401 && loginUrl && !redirecting) {
      redirecting = true
      window.location.assign(loginHref(loginUrl))
    }
    throw new ApiRequestError(response.status, message, loginUrl)
  }
  return payload as T
}

export function errorMessage(error: unknown): string {
  if (error instanceof ApiRequestError) return error.message
  if (error instanceof Error && error.message) return error.message
  return 'Une erreur inattendue est survenue.'
}

/** Lien d'export CSV (téléchargement direct, même origine). */
export function csvUrl(path: string, query?: Query): string {
  return `/api${path}${toQueryString(query)}`
}
