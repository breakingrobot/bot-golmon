// Service worker minimal (application installable) : servi sans cache long par le bot (no-cache).
//
// - `/assets/*` (noms avec empreinte, immuables) : cache d'abord, gardé entre les versions puis
//   élagué à l'activation d'une nouvelle version du service worker.
// - Navigation : réseau d'abord ; hors ligne, la page `offline.html` mise en cache à l'installation.
// - Tout le reste, et jamais `/api/`, `/login`, `/logout`, `/auth/` : le navigateur seul, sans cache.

const VERSION = 'v1'
const ASSETS = `assets-${VERSION}`
const PAGES = `pages-${VERSION}`
const OFFLINE = '/offline.html'

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches
      .open(PAGES)
      .then((cache) => cache.addAll([OFFLINE, '/favicon.svg']))
      .then(() => self.skipWaiting()),
  )
})

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(keys.filter((key) => key !== ASSETS && key !== PAGES).map((key) => caches.delete(key))),
      )
      .then(() => self.clients.claim()),
  )
})

self.addEventListener('fetch', (event) => {
  const { request } = event
  if (request.method !== 'GET') return
  const url = new URL(request.url)
  if (url.origin !== self.location.origin) return
  // Données et connexion : jamais par le service worker.
  if (/^\/(api|login|logout|auth)(\/|$)/u.test(url.pathname)) return

  if (url.pathname.startsWith('/assets/')) {
    event.respondWith(
      caches.open(ASSETS).then(async (cache) => {
        const cached = await cache.match(request)
        if (cached) return cached
        const response = await fetch(request)
        if (response.ok) await cache.put(request, response.clone())
        return response
      }),
    )
    return
  }

  if (request.mode === 'navigate') {
    event.respondWith(fetch(request).catch(() => caches.match(OFFLINE).then((page) => page ?? Response.error())))
  }
})
