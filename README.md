# Bot Discord — Transactions & Présences

## Setup

```bash
pnpm install
cp .env.example .env   # renseigner DISCORD_TOKEN, DISCORD_CLIENT_ID, DISCORD_GUILD_ID
pnpm db:migrate
pnpm deploy-commands    # enregistre les slash commands sur la guilde configurée
pnpm dev
```

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
