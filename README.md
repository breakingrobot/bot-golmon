# Bot Discord — Transactions & Présences

## Setup

```bash
pnpm install
cp .env.example .env   # renseigner DISCORD_TOKEN, DISCORD_CLIENT_ID, DISCORD_GUILD_IDS
pnpm db:migrate
pnpm deploy-commands    # enregistre les slash commands sur chaque guilde whitelistée
pnpm dev
```

## Multi-serveur (whitelist)

Le bot est multi-guild : chaque `Transaction`/`Presence`/`PresenceConfig` est scopée par `guildId`, aucune donnée n'est partagée entre serveurs.

`DISCORD_GUILD_IDS` (liste d'IDs séparés par des virgules) contrôle les serveurs autorisés :
- si renseigné : les commandes sont déployées uniquement sur ces guildes, et le bot **quitte automatiquement** tout serveur non listé (à l'ajout via `guildCreate`, et au démarrage via un balayage dans `ready`).
- si vide : aucune restriction (pratique en dev), déploiement global des commandes.

## Scripts

- `pnpm dev` — lance le bot en mode watch
- `pnpm build` / `pnpm start` — build puis lance en production
- `pnpm deploy-commands` — (re)déploie les slash commands
- `pnpm lint` / `pnpm lint:fix` — Oxc
- `pnpm db:migrate` / `pnpm db:studio` — Prisma

## Modules

- **Transactions** (`/transaction-*`) : suivi achats/ventes avec embed de statut par salon.
- **Présences** (`/presence-*`) : convocations avec boutons Présent/Absent/Retard, rappels automatiques toutes les 3h, clôture auto à l'heure de l'événement, stats par joueur.

Configurer le module présences avec `/presence-config` avant la première utilisation.
