# Interface web (React)

Application monopage du site du bot : statistiques des serveurs, transactions, cotisations, coffres, contacts,
journal du staff, fiches des membres, permissions, configuration, tickets et logs. Le bot (axum) sert les
fichiers construits de `ui/dist` et l'API JSON sous `/api`, sur la même origine.

Le contrat de l'API est `src/api/types.ts` ; la spécification OpenAPI du bot (`ui/openapi.json`, aussi servie sur
`/api/openapi.json`) en est la version exportée.

## Pile

| Outil                                      | Version                                         | Rôle                                                            |
| ------------------------------------------ | ----------------------------------------------- | --------------------------------------------------------------- |
| React                                      | 19.3.0                                          | Interface                                                       |
| Vite                                       | 8.3.2                                           | Serveur de développement, compilation (`dist/`)                 |
| TanStack Router                            | 1.170.41 (+ `@tanstack/router-plugin` 1.168.42) | Routes par fichiers (`src/routes`), découpage du code par route |
| TanStack Query                             | 5.104.1                                         | Toutes les requêtes à l'API                                     |
| Tailwind CSS                               | 4.3.3                                           | Styles, jetons du thème dans `src/styles.css`                   |
| shadcn/ui (Radix, paquet `radix-ui` 1.6.7) | —                                               | Composants (`src/components/ui`), écrits dans le style shadcn   |
| Recharts                                   | 3.10.1                                          | Graphiques, chargés seulement par les pages qui en ont          |
| lucide-react                               | 1.52.0                                          | Icônes                                                          |
| sonner                                     | 2.0.8                                           | Notifications (toasts)                                          |
| TypeScript                                 | 7.0.2 (compilateur natif)                       | `npm run typecheck` (`tsc --noEmit`) ; Vite transpile           |
| Oxlint                                     | 1.86.0                                          | Lint (`.oxlintrc.json`)                                         |
| Oxfmt                                      | 0.71.0                                          | Format (`.oxfmtrc.json`)                                        |
| Hey API (`@hey-api/openapi-ts`)            | 0.0.0-next-20260930190945 (étiquette `next`)    | Génération du client typé depuis `openapi.json`                 |

Versions exactes, publiées au moins deux semaines avant leur adoption. Hey API est pris sur l'étiquette `next` : la
version stable ne prend pas encore TypeScript 7 en charge. Le client fetch de Hey API est généré dans le dossier de
sortie (pas de paquet `@hey-api/client-fetch` à installer).

**Pourquoi pas TanStack Start (mode SPA) ?** Il compile, mais la page d'amorce qu'il pré-rend contient des scripts en
ligne (données d'hydratation `$_TSR`, restauration du défilement) indispensables au démarrage. La CSP du bot
(`script-src 'self'`) les bloque, et leurs empreintes changent à chaque compilation. Vite + TanStack Router produit
un `index.html` sans script en ligne et ne construit pas de serveur Node inutile.

TypeScript strict : `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noImplicitOverride`,
`noFallthroughCasesInSwitch`, `noUnusedLocals`, `noUnusedParameters`, `verbatimModuleSyntax`, `isolatedModules`. Pas
de `any`, pas d'assertion non nulle.

## Scripts

```sh
npm ci                 # dépendances (package-lock.json versionné)
npm run dev            # développement, API relayée vers le bot (BOT_URL, http://localhost:8890 par défaut)
npm run dev:mock       # développement sans le bot : maquette de l'API
npm run build          # compilation dans dist/
npm run preview:mock   # compilation en maquette (dist-mock/) et aperçu sur http://localhost:4173
npm run typecheck      # tsc --noEmit (TypeScript 7)
npm run lint           # oxlint, avertissements = erreurs
npm run format         # oxfmt (écrit)
npm run format:check   # oxfmt --check
npm run generate:api   # client Hey API depuis openapi.json → src/api/generated (non versionné)
npm run check:api      # génère, puis vérifie que la spécification convient aux types de src/api/types.ts
```

`lint`, `format:check`, `typecheck` et `build` doivent passer avant chaque commit. Les fichiers générés
(`src/routeTree.gen.ts`, `src/api/generated/`) sont exclus du lint et du format ; `src/routeTree.gen.ts` est
versionné (il est réécrit par Vite à chaque lancement).

## Mode maquette

`VITE_MOCK=1` remplace l'API par un faux serveur en mémoire (`src/mocks/`) : un groupe RP fictif (membres,
transactions, cotisations, coffres, contacts, journal, tickets, logs). Toutes les routes du contrat répondent, y
compris les écritures (ajout de cotisation, contacts, notes, permissions, objectif, alertes…), gardées en mémoire
jusqu'au rechargement. La condition est remplacée à la compilation : en production, le code de la maquette n'est
pas dans `dist/`.

Scénarios de connexion par paramètre d'adresse (mémorisé pour l'onglet) : `?mock=admin` (par défaut), `?mock=membre`
(simple membre), `?mock=aucun` (connecté sans accès), `?mock=anon` (pas connecté). Les exports CSV sont des liens
directs vers le bot : sans effet en maquette.

## Structure

```
src/
  api/          types.ts (contrat), client.ts (fetch + erreurs + 401), queries.ts (options TanStack Query
                et mutations, seul point d'accès à l'API), hey-api.ts (configuration du client généré),
                contract-check.ts (spécification ↔ types)
  routes/       une route par fichier (TanStack Router), redirections des anciennes adresses comprises
  components/   ui/ (shadcn), charts/ (Recharts), mise en page, formulaires, dialogues
  lib/          format (Intl, « 12 500 $ »), thème, droits par page, paramètres d'adresse
  mocks/        maquette de l'API (exclue du build)
public/         theme-init.js (thème avant le premier rendu), favicon
```

Les pages n'appellent jamais `fetch` : elles importent les options de `src/api/queries.ts`. Passer aux types et
au client générés par Hey API ne touche que `src/api/`.

## Routes

| Adresse                                                        | Page                                                                                        |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `/`                                                            | Connexion, ou tuiles des serveurs et des tickets                                            |
| `/serveurs/:id`                                                | Statistiques (`site.stats`)                                                                 |
| `/serveurs/:id/transactions`                                   | Transactions, filtres dans l'adresse, export CSV (`transaction.historique`)                 |
| `/serveurs/:id/cotisations`                                    | Cotisations, objectif, ajout (`cotisation.stats`, `cotisation.ajouter`, `cotisation.gerer`) |
| `/serveurs/:id/coffres`                                        | Stock, évolution d'un objet, joueurs, mouvements (`coffre.voir`, `coffre.gerer`)            |
| `/serveurs/:id/contacts`                                       | Répertoire, fiche et notes, création / modification (`contact.*`)                           |
| `/serveurs/:id/journal`                                        | Journal du staff (`site.journal`)                                                           |
| `/serveurs/:id/membres`, `/serveurs/:id/membres/:user`         | Recherche et fiche d'un membre (`membre.fiche` ; sa propre fiche pour tous)                 |
| `/serveurs/:id/permissions`, `/serveurs/:id/configuration`     | Réglages (« Gérer le serveur »)                                                             |
| `/tickets`, `/tickets/staff`, `/logs/:thread`                  | Tickets, statistiques du staff, log d'un ticket                                             |
| `/membres/:user?serveur=:id`                                   | Redirection vers la fiche (liens envoyés par le bot)                                        |
| `/stats…`, `/cotisations`, `/contacts`, `/logs`, `/logs/staff` | Redirections des anciennes adresses                                                         |

## Service par le bot

- Fichiers de `dist/` servis tels quels (`/assets/*` avec un cache long : noms avec empreinte ; `index.html`,
  `theme-init.js`, `favicon.svg` sans cache long).
- Toute adresse inconnue hors `/api`, `/login`, `/logout`, `/auth/*`, `/healthz` et `/logs/:id?raw=1` renvoie
  `index.html` (routage côté client).
- CSP compatible : aucun script ni style en ligne dans `index.html`. Il faut `connect-src 'self'` pour l'API ;
  `style-src 'self'` suffit (sonner et Radix tentent d'injecter des `<style>`, refusés sans gêne : les styles de
  sonner sont déjà dans la feuille compilée), `img-src` doit accepter les avatars Discord et les pièces jointes, et
  `font-src 'self'` la police Inter auto-hébergée.
- Écritures : en-têtes `Content-Type: application/json` et `X-Requested-With: fetch`. Sur 401, l'interface suit
  `loginUrl` en remplaçant `next` par l'adresse affichée.
