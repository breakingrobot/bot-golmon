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

### Présences

À configurer **une seule fois** ; ensuite plus rien n'est redemandé à la création :

```
/presence config role:@AOD exception_role:@Amis channel:#présences
/presence create heure:21:00
```

- `/presence config` sans option affiche la configuration actuelle. La lecture est ouverte à tous, la modification demande « Gérer le serveur ».
- `/presence create` : seule `heure` est requise. `date` vaut aujourd'hui par défaut (bascule sur demain si l'heure est déjà passée), `titre` vaut « Présence ». Le rôle ciblé, le rôle d'exception et le salon viennent de la config.
- Boutons Présent / Absent / Retard (modal pour la durée) / Ping. L'embed affiche aussi les membres **sans réponse**.
- Rappels automatiques des non-répondants, au plus une fois toutes les 3h par présence ; clôture automatique à l'heure de l'événement (contrôle toutes les 5 min).
- `/presence stats [joueur]` : fiche individuelle (avec l'historique des changements d'avis) ou classement du serveur.

### Transactions

```
/transaction config dashboard_channel:#transactions
/transaction create groupe:X type:Vente prix:1500
```

- Tableau de bord permanent auto-actualisé dans le salon configuré : toutes les transactions groupées par statut, avec total engagé.
- L'embed détaillé de chaque transaction est posté dans le salon d'invocation, avec des **boutons de changement de statut** (le statut courant est désactivé).
- `/transaction add-item`, `status`, `view`, `cancel` avec autocomplétion.

## Fuseau horaire

Les saisies sont interprétées en **Europe/Paris** (`src/lib/datetime.ts`), indépendamment du fuseau du serveur d'hébergement — le conteneur de prod tourne en UTC. L'affichage utilise les timestamps Discord (`<t:…>`), donc chaque utilisateur voit l'heure dans son propre fuseau.
