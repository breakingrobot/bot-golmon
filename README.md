# Bot Discord — Présences, Transactions, Véhicules, Recrues, Numéros & Tickets

Bot écrit en **Rust** avec [Twilight](https://twilight.rs) : bibliothèque Discord modulaire (gateway,
HTTP, cache séparés), cache filtré type par type et événements filtrés avant désérialisation. Le bot
ne garde en mémoire que les guildes, salons, rôles et membres.

| Brique | Rôle |
|---|---|
| `twilight-gateway` / `twilight-http` / `twilight-cache-inmemory` | Connexion Discord, API REST (rate limits gérés), cache minimal |
| `sqlx` (SQLite, WAL) | Base de données, migrations versionnées dans `migrations/` |
| `tokio` | Runtime asynchrone, tâches périodiques |
| `axum` + `askama` | Serveur web des logs de tickets (templates compilés) |
| Tailwind CSS v4 | Styles des pages de logs, compilés au build et embarqués dans le binaire |

## Démarrage

```bash
cp .env.example .env    # renseigner DISCORD_TOKEN et DISCORD_GUILD_IDS
cargo run --release     # applique les migrations, déploie les slash commands, démarre le bot
```

- `bot-golmon deploy-commands` déploie seulement les slash commands puis s'arrête.
- `DEPLOY_COMMANDS=false` désactive le déploiement automatique au démarrage.
- Intents à activer dans le portail développeur Discord : **Server Members**, et **Message Content**
  si le module ticket est utilisé.

### Variables d'environnement

| Variable | Défaut | Description |
|---|---|---|
| `DISCORD_TOKEN` | — | Token du bot (requis) |
| `DISCORD_GUILD_IDS` | vide | Guildes autorisées, séparées par des virgules. Vide = aucune restriction |
| `DATABASE_URL` | `file:./prisma/dev.db` | Base SQLite principale (même chemin que la version TypeScript) |
| `DEPLOY_COMMANDS` | `true` | Déploie les slash commands au démarrage |
| `RUST_LOG` | `info` | Niveau de logs |
| `MM_*`, `PORT` | — | Configuration du module ticket (voir plus bas) |

## Déploiement (Northflank)

L'image Docker se construit avec le `Dockerfile` du dépôt, sans étape supplémentaire :

1. **CSS** : Node compile la feuille Tailwind.
2. **Build** : Rust compile le binaire, les dépendances étant mises en cache dans une couche dédiée.
3. **Runtime** : `debian:trixie-slim` avec le seul binaire (~20 Mo). Les migrations s'appliquent au démarrage.

À configurer sur le service :

- les variables d'environnement ci-dessus ;
- un **volume persistant** pour la base. Avec le `DATABASE_URL` par défaut, monter le volume sur
  `/app/prisma` (même emplacement que l'ancienne image : la base existante est reprise telle quelle) ;
- si le module ticket est actif : exposer le **port 8890** (ou `PORT`) en HTTP public, et renseigner
  `MM_URL` avec l'URL publique, pour que les liens de logs pointent au bon endroit. Le chemin
  `/healthz` peut servir de health check. Monter aussi le dossier de la base modmail (`db/` par défaut)
  sur un volume.

### Reprise d'une base existante

Au premier démarrage, le bot reprend la base SQLite de la version TypeScript **sans perte** :

- le schéma Prisma est repris tel quel (migration `0001` en `IF NOT EXISTS`) ;
- les tables `Prospect*` sont renommées en `Recrue*` (`0002`), données, avis, vétos et salons d'essai compris ;
- les éventuelles dates stockées en millisecondes sont converties en ISO 8601 (`0003`).

Les commandes `/prospect` disparaissent au redéploiement des commandes, remplacées par `/recrue`.

## Multi-serveur (liste blanche)

Chaque donnée est scopée par `guildId`, rien n'est partagé entre serveurs. Si `DISCORD_GUILD_IDS` est
renseigné, les commandes sont déployées uniquement sur ces guildes et le bot **quitte automatiquement**
tout autre serveur (à l'arrivée et au démarrage). Vide : aucune restriction, déploiement global.

## Modules

### Présences

À configurer **une seule fois** ; ensuite plus rien n'est redemandé à la création :

```
/presence config role:@AOD absence_role:@Absent channel:#présences absence_channel:#absences
/presence create heure:21:00
```

- `/presence config` sans option affiche la configuration actuelle (réservé à « Gérer le serveur »).
- `/presence create` : seule `heure` est requise. `date` vaut aujourd'hui par défaut (bascule sur demain si l'heure est déjà passée), `titre` vaut « Présence ».
- Boutons Présent / Absent / Retard (modal pour la durée) / Ping. L'embed affiche aussi les membres **sans réponse**.
- Rappels automatiques des non-répondants, au plus une fois toutes les 3 h par présence ; clôture automatique à l'heure de l'événement (contrôle toutes les 5 min).
- `/presence stats [joueur]` : fiche individuelle (avec l'historique des changements d'avis) ou classement du serveur.
- `/presence forcevote utilisateur statut [presence] [retard]` : le staff saisit la réponse d'un membre. L'autocomplétion inclut les présences clôturées (🔒) pour corriger une statistique.
- `/presence ping`, `close`, `changes`, `cancel`, `reset` complètent le module.

### Absences

```
/absence declare motif:"Vacances" fin:20/08/2026
```

- `debut` vaut aujourd'hui par défaut ; une date couvre la journée entière (00:00 → 23:59).
- Le **rôle d'absence** est appliqué automatiquement puis retiré à la fin de la période (contrôle toutes les 5 min). En cas d'absences qui se chevauchent, le rôle n'est retiré qu'à la fin de la dernière.
- Les présences ouvertes tombant dans la période passent automatiquement en **Absent**.
- L'absence est annoncée dans le salon `absence_channel`.
- `/absence end [membre]` y met fin par anticipation, `/absence list` liste les absences en cours.
- Le staff (« Gérer les messages ») peut déclarer ou clore l'absence d'un autre membre.

Si le bot ne peut pas appliquer le rôle (permission « Gérer les rôles » manquante ou rôle au-dessus du
sien), l'absence est **quand même enregistrée** et le problème est signalé.

### Blocklist présences

```
/presence blocklist add membre:@Membre raison:"Inactif"
```

Les membres de la blocklist ne sont **jamais pingés** et sont marqués absents d'office à chaque
nouvelle présence. `/presence blocklist list` et `remove` complètent le groupe.

### Transactions

```
/transaction config dashboard_channel:#transactions
/transaction create groupe:X type:Vente prix:1500
```

- Tableau de bord permanent auto-actualisé : transactions groupées par statut, avec le total engagé.
- L'embed de chaque transaction est posté dans le salon d'invocation, avec des **boutons de statut** (le statut courant est désactivé), d'ajout d'objet et de suppression.
- `/transaction add-item`, `status`, `view`, `cancel`, `delete` avec autocomplétion.
- `ping_role` mentionne un rôle à la création ; `clear_ping_role` le retire.

### Véhicules

```
/vehicle config dashboard_channel:#véhicules
/vehicle add plaque:D6CS3P0G proprietaire:@Membre
/vehicle lookup plaque:D6CS3P0G
```

- Tableau de bord permanent, **groupé par propriétaire** (membre ou rôle).
- `/vehicle add` récupère le modèle via l'API GLife ; si elle est injoignable, l'ajout réussit quand même.
- `/vehicle lookup` interroge l'API pour **n'importe quelle plaque** : modèle, propriétaire en jeu, acheteur, statut illégal.
- `/vehicle owner` transfère un véhicule, `/vehicle remove` le retire.
- Plaques normalisées (majuscules, sans espaces ni tirets) : impossible de créer un doublon.

API : `GET https://api.glife.fr/roleplay/vehicles?plate=…`, cache de 5 min, timeout de 8 s, requêtes
concurrentes mutualisées.

### Recrues

Avis hebdomadaires des gradés sur les membres en période d'essai, avec droit de véto des leads.

```
/recrue config role_recrue:@Recrue role_grade:@Gradé role_lead:@Lead categorie:#Essais salon:#avis
/recrue liste
/recrue avis membre:@Membre
/recrue veto poser membre:@Membre motif:Comportement inadapté
```

- **Un embed par semaine** dans le salon configuré, listant chaque recrue avec le décompte `✅ / ❌ / ⬜` et les motifs. Trois menus : donner son avis, poser un véto, consulter le détail.
- Le vote passe par une **modal** (menu Pour / Contre / Blanc **et motif obligatoire**). Un gradé n'a qu'un avis par recrue : revoter remplace le précédent.
- **Reset chaque lundi 00:00** (Europe/Paris) : la semaine écoulée est archivée et son embed reste visible ; un rattrapage quotidien couvre un bot hors ligne au moment de la bascule. `/recrue historique` consulte les semaines passées.
- **Salon privé automatique** : dès qu'un membre reçoit le rôle recrue, un salon `essai-<pseudo>` est créé dans la catégorie configurée, visible uniquement par lui, les gradés et les leads. `/recrue salon` fait la même chose manuellement. Perdre le rôle retire le membre de la liste de la semaine mais **conserve son salon**.
- **Véto** : réservé au rôle lead, motif obligatoire, affiché en rouge. `/recrue veto lever` le retire. « Gérer le serveur » ne donne **pas** le droit de véto ; cette permission sert à `/recrue config` et `/recrue reset`.

Le bot doit disposer de la permission **Gérer les salons** pour créer les salons d'essai.

### Numéros

```
/numero config salon:#annuaire
/numero ajouter numero:5551234
/numero voir membre:@Membre
```

- Embed permanent avec un bouton **« Ajouter / modifier mon numéro »** (modal) et un bouton pour retirer le sien.
- Format : `555` suivi de **4 à 6 chiffres**. `555-1234` et `5551234` sont équivalents.
- **Un numéro par membre**, et un numéro ne peut pas être attribué à deux membres.
- `/numero ajouter numero:… membre:@Membre` inscrit **de force** le numéro d'un autre membre (« Gérer le serveur »).

## Tickets (modmail)

Réimplémentation de [Dragory/modmailbot](https://github.com/Dragory/modmailbot) : les membres écrivent
au bot en message privé, un salon de ticket est ouvert sur le serveur inbox et le staff répond depuis
ce salon.

### Compatibilité

- **Configuration** : mêmes options et valeurs par défaut, lues depuis `config.ini` (ou `config.json`,
  ou le fichier pointé par `MODMAIL_CONFIG`) et depuis les variables `MM_*` (`MM_CLOSE_MESSAGE` →
  `closeMessage`, `MM_COMMAND_ALIASES__MV` → `commandAliases.mv`, `||` pour les listes). Un
  `config.ini` modmailbot existant fonctionne tel quel. Voir `config.example.ini`.
- **Commandes** : mêmes noms, alias, arguments et messages : `!reply`/`!r`, `!anonreply`/`!ar`,
  `!realreply`/`!rr`, `!edit`, `!delete`, `!close [-s] [délai|cancel]`, `!logs`, `!loglink`, `!block`,
  `!unblock`, `!is_blocked`, `!blocklist`, `!move`, `!suspend`, `!unsuspend`, `!alert`, `!id`,
  `!dm_channel_id`, `!message`, `!role`, `!note`, `!notes`, `!delete_note`, `!newthread`, snippets
  (`!s`, `!es`, `!ds`, `!snippets`, `!!trigger`, `!!!trigger`, `{{trigger}}` en ligne), `commandAliases`…
  Référence : [documentation des commandes de modmailbot](https://github.com/Dragory/modmailbot/blob/master/docs/commands.md).
- **Base de données** : schéma identique, migrations aux mêmes noms dans `knex_migrations`. Une base
  `db/data.sqlite` de modmailbot est reprise sans modification, et reste lisible par modmailbot.
- **Logs** : `logStorage` `local` (page web), `attachment` (fichier joint) ou `none` ; pièces jointes
  `attachmentStorage` `original`, `local` ou `discord`.

Non repris : les plugins JavaScript, les notifications de mise à jour de modmailbot et `dbType = mysql`
(SQLite uniquement).

### Activer le module

Le module est inactif tant qu'aucun `config.ini` ni variable `MM_*` n'est présent. Minimum :

```bash
MM_MAIN_SERVER_ID=111111111111111111   # serveur(s) des membres (plusieurs : 111||222)
MM_INBOX_SERVER_ID=111111111111111111  # serveur où sont créés les salons de tickets
MM_LOG_CHANNEL_ID=222222222222222222   # salon des notifications de fermeture
MM_URL=https://tickets.example.com     # URL publique du serveur web (liens de logs)
```

Le token est celui du bot (`DISCORD_TOKEN`). La base modmail est stockée dans `db/data.sqlite`
(`MM_SQLITE_OPTIONS__FILENAME` pour la déplacer).

### Logs de tickets

`/logs/<id du ticket>` affiche le ticket dans une page lisible (thème clair/sombre automatique, mobile) :
messages de l'utilisateur, réponses du staff (numéro, rôle, anonymat), notes internes, commandes,
réponses modifiées (avant/après) ou supprimées, images en aperçu, fichiers, embeds, markdown Discord.
Filtres « Échanges seulement » (`?simple=1`) et « Détails techniques » (`?verbose=1`, identifiants des
messages privés). `?raw=1` renvoie le log texte au format exact de modmailbot.

Les couleurs sont des **tokens sémantiques** (`web/src/logs.css`) : valeurs brutes sur `:root`
(redéfinies en sombre) exposées à Tailwind via `@theme inline` (`bg-surface`, `text-ink-muted`,
`border-l-staff`…). Pour modifier le thème, changer ces valeurs puis recompiler :

```bash
cd web && npm ci && npm run build   # régénère web/dist/logs.css (embarqué à la compilation)
```

## Fuseau horaire

Les saisies sont interprétées en **Europe/Paris** (`src/util/time.rs`), heure d'été comprise,
indépendamment du fuseau du serveur (le conteneur tourne en UTC). L'affichage utilise les timestamps
Discord (`<t:…>`) : chacun voit l'heure dans son propre fuseau.

## Développement

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

| Dossier | Contenu |
|---|---|
| `src/discord/` | Gateway, aiguillage des interactions, réponses, options, composants, cache des membres |
| `src/modules/` | Un dossier par module à slash commands (`commands`, `service`, `embed`, `interactions`, `cron`) |
| `src/modmail/` | Module ticket : config, base knex, tickets, commandes texte, événements, serveur web |
| `src/util/` | Dates (Europe/Paris), mise en forme bornée aux limites Discord, identifiants |
| `migrations/` | Migrations SQL de la base principale |
| `templates/`, `web/` | Page de logs (Askama) et sa feuille de style Tailwind |
