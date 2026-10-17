# Déploiement

Au démarrage, le bot crée ou met à jour sa base, déploie les slash commands puis se connecte à Discord.
Toutes ses données sont dans **`/app/data`** : un seul volume à monter.

## Northflank

1. **Create new → Service → Combined service**.
2. **Repository** : ce dépôt et la branche à déployer. **Build type** : *Dockerfile*, chemin `/Dockerfile`,
   contexte `/`.
3. **Environment variables** : `DISCORD_TOKEN` et `DISCORD_GUILD_IDS` au minimum ([variables](#variables-denvironnement)).
   Mettre le token dans un *Secret group*.
4. **Volumes → Add volume** : chemin de montage **`/app/data`**, la plus petite taille suffit.
5. **Ports** : seulement avec le module ticket, voir [HTTPS](#https-module-ticket).
6. **Resources** : le plus petit plan suffit.
7. Déployer, puis vérifier dans **Logs** : `connecté à Discord`.

Un volume limite le service à une instance, ce qui est voulu : un bot Discord ne tourne qu'en un exemplaire.
Pour mettre à jour, pousser sur la branche suivie : Northflank reconstruit et redéploie.

## HTTPS (module ticket)

Le bot sert les pages de logs des tickets en HTTP simple sur le port `8890`. Le HTTPS est assuré par
Northflank, qui termine le TLS devant le service : rien à configurer dans le bot.

1. Dans le service → **Ports & DNS → Add port** : port `8890`, protocole **HTTP**, **Publicly expose**
   coché.
2. Northflank attribue une adresse `https://….code.run` avec un certificat valide. La mettre dans
   `MM_URL`, sans `/` final, puis redéployer.
3. *Health check* (facultatif) : HTTP, port `8890`, chemin `/healthz`.

**Domaine personnalisé** (ex. `tickets.mon-domaine.fr`) :

1. **Team settings → Domains → Add domain**, puis créer chez le registraire l'enregistrement DNS demandé
   pour la vérification.
2. Ajouter le sous-domaine, créer l'enregistrement `CNAME` indiqué, puis le lier au port `8890` du service
   (dans le domaine, ou dans **Ports & DNS** du service).
3. Le certificat Let's Encrypt est généré automatiquement. Mettre `MM_URL=https://tickets.mon-domaine.fr`.

Hors Northflank, placer un reverse proxy devant le port 8890 (par exemple Caddy :
`tickets.mon-domaine.fr { reverse_proxy localhost:8890 }`, certificat automatique).

## Docker

```bash
docker build -t bot-discord .
docker run -d --name bot-discord --restart unless-stopped \
  --env-file .env -v bot-data:/app/data -p 8890:8890 bot-discord
```

`-p 8890:8890` ne sert qu'au module ticket.

## En local

Prérequis : [Rust](https://rustup.rs) stable.

```bash
cp .env.example .env      # renseigner DISCORD_TOKEN et DISCORD_GUILD_IDS
cargo run --release
```

## Variables d'environnement

| Variable | Défaut | Description |
|---|---|---|
| `DISCORD_TOKEN` | — | **Requis.** Token du bot. |
| `DISCORD_GUILD_IDS` | *(vide)* | Serveurs autorisés, séparés par des virgules. Le bot quitte les autres serveurs et y déploie ses commandes immédiatement. Vide : aucune restriction, commandes globales (jusqu'à une heure de propagation). **À renseigner en production.** |
| `DATABASE_URL` | `file:./data/bot.db` | Base SQLite principale. |
| `DEPLOY_COMMANDS` | `true` | Déployer les slash commands au démarrage. `bot-discord deploy-commands` les déploie puis s'arrête. |
| `RUST_LOG` | `info` | Détail des logs : `error`, `warn`, `info`, `debug`. |
| `MM_*`, `MODMAIL_CONFIG`, `PORT` | — | [Module ticket](tickets.md). |

Les heures sont toujours interprétées en heure de Paris, quel que soit `TZ`.

## Contenu de `/app/data`

| Fichier | Contenu |
|---|---|
| `bot.db` | Base principale |
| `modmail.sqlite` | Base des tickets |
| `logs/`, `attachments/` | Seulement avec `logStorage = attachment` ou `attachmentStorage = local` |

## Reprendre les données de l'ancien bot (TypeScript)

Copier son fichier `dev.db` dans le volume `/app/data` **avant le premier démarrage**. Le bot le renomme
en `bot.db` et le met à jour sans perte (tables `Prospect*` renommées en `Recrue*`, dates normalisées).

- `/prospect` est remplacée par `/recrue` (option `role_prospect` → `role_recrue`) ;
- les messages déjà publiés (présences, tableaux de bord, avis de la semaine) restent fonctionnels.

Pour reprendre une installation modmailbot, voir [Tickets](tickets.md#reprendre-une-installation-modmailbot).
