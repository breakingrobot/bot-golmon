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
5. **Ports & DNS → Add port** : port `8890`, protocole **HTTP**, **Publicly expose** coché (voir
   [site web](#site-web-et-https)). *Health check* : HTTP, port `8890`, chemin `/healthz`.
6. **Resources** : le plus petit plan suffit.
7. Déployer, puis vérifier dans **Logs** : `connecté à Discord` et `serveur web démarré`.

Un volume limite le service à une instance, ce qui est voulu : un bot Discord ne tourne qu'en un exemplaire.
Pour mettre à jour, pousser sur la branche suivie : Northflank reconstruit et redéploie.

## Site web et HTTPS

Le bot sert un petit site sur le port `8890` : logs et recherche des tickets, statistiques du staff,
statistiques des serveurs, et `/healthz` pour la plateforme. Il parle HTTP simple ; le HTTPS est assuré
par Northflank, qui termine le TLS devant le service.

1. Port `8890` exposé publiquement en **HTTP** (étape 5 ci-dessus).
2. Northflank attribue une adresse `https://….code.run` avec un certificat valide. La mettre dans
   `WEB_URL`, sans `/` final (et dans `MM_URL` si le module ticket est utilisé).

**Domaine personnalisé** (ex. `bot.mon-domaine.fr`) :

1. **Team settings → Domains → Add domain**, puis créer chez le registraire l'enregistrement DNS demandé
   pour la vérification.
2. Ajouter le sous-domaine, créer l'enregistrement `CNAME` indiqué, puis le lier au port `8890` du service.
3. Le certificat Let's Encrypt est généré automatiquement. Mettre `WEB_URL=https://bot.mon-domaine.fr`.

Hors Northflank, placer un reverse proxy devant le port 8890 (par exemple Caddy :
`bot.mon-domaine.fr { reverse_proxy localhost:8890 }`, certificat automatique).

### Connexion Discord

Les pages sont réservées aux personnes connectées avec leur compte Discord :

| Page | Accès |
|---|---|
| `/logs`, `/logs/<id>`, `/logs/staff` | Staff des tickets (`inboxServerPermission` sur le serveur inbox) |
| `/stats/<serveur>` | Membres ayant « Gérer le serveur » |

Pour l'activer :

1. Portail développeur Discord → l'application → **OAuth2** : **Reset Secret**, copier le *Client Secret*
   dans `DISCORD_CLIENT_SECRET` (dans le *Secret group*).
2. Même page, **Redirects → Add Redirect** : `https://<WEB_URL>/auth/callback` (l'adresse exacte de
   `WEB_URL` suivie de `/auth/callback`), puis **Save Changes**.
3. Redéployer et ouvrir `WEB_URL` : bouton **Se connecter**.

La session dure 7 jours. Les droits sont vérifiés à chaque page : retirer un rôle coupe l'accès
immédiatement.

**Sans `DISCORD_CLIENT_SECRET`**, les logs restent lisibles par leur lien (non devinable, comme dans
modmailbot) et les autres pages sont désactivées. À éviter en production : un lien de log transféré
suffit alors pour le lire.

## Docker

```bash
docker build -t bot-discord .
docker run -d --name bot-discord --restart unless-stopped \
  --env-file .env -v bot-data:/app/data -p 8890:8890 bot-discord
```

`-p 8890:8890` expose le site web (logs, statistiques, `/healthz`).

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
| `WEB_URL` | `MM_URL` | Adresse publique du site, sans `/` final (ex. `https://bot.mon-domaine.fr`). |
| `DISCORD_CLIENT_SECRET` | — | Active la [connexion Discord](#connexion-discord) du site. |
| `PORT` | `8890` | Port du site web (l'option `port` du module ticket a priorité). |
| `MM_*`, `MODMAIL_CONFIG` | — | [Module ticket](tickets.md). |

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
