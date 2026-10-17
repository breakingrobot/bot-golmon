# Bot Golmon

Bot Discord pour la gestion d'un groupe RP : **présences et absences**, **transactions**, **registre des
véhicules**, **suivi des recrues** (avis et vétos hebdomadaires), **annuaire téléphonique** et **tickets**
(modmail compatible [Dragory/modmailbot](https://github.com/Dragory/modmailbot)).

Écrit en Rust avec [Twilight](https://twilight.rs) et SQLite. Le bot ne garde en mémoire que le strict
nécessaire (guildes, salons, rôles, membres) et se déploie avec un simple `Dockerfile`.

## Sommaire

1. [Créer le bot sur Discord](#1-créer-le-bot-sur-discord)
2. [Lancer le bot](#2-lancer-le-bot) — en local, avec Docker, ou [sur Northflank](#déploiement-sur-northflank)
3. [Variables d'environnement](#3-variables-denvironnement)
4. [Configurer les modules sur Discord](#4-configurer-les-modules-sur-discord)
5. [Tickets (modmail)](#5-tickets-modmail)
6. [Mise à jour depuis la version TypeScript](#6-mise-à-jour-depuis-la-version-typescript)
7. [Dépannage](#7-dépannage)
8. [Développement](#8-développement)

---

## 1. Créer le bot sur Discord

### Application et token

1. Ouvrir le [portail développeur Discord](https://discord.com/developers/applications) → **New Application**.
2. Onglet **Bot** → **Reset Token** : copier le token, c'est la valeur de `DISCORD_TOKEN`.
   Ne jamais le partager ni le committer ; s'il fuite, le régénérer immédiatement.
3. Toujours dans **Bot**, section *Privileged Gateway Intents*, activer :

   | Intent | Nécessaire pour |
   |---|---|
   | **Server Members Intent** | Toujours : rôles des présences, recrues, ping des non-répondants |
   | **Message Content Intent** | Uniquement si le module ticket est activé (commandes `!reply`, `!close`…) |

   *Presence Intent* n'est pas utilisé.

### Inviter le bot

Onglet **OAuth2 → URL Generator** : cocher les scopes `bot` et `applications.commands`, puis les
permissions ci-dessous. Ou utiliser directement ce lien en remplaçant `CLIENT_ID` par l'*Application ID* :

```
https://discord.com/oauth2/authorize?client_id=CLIENT_ID&scope=bot+applications.commands&permissions=2251800082377808
```

| Permission | Utilisée par |
|---|---|
| Voir les salons, Envoyer des messages, Intégrer des liens, Joindre des fichiers, Voir l'historique | Tous les modules |
| Gérer les messages | Suppression des messages de transaction et des commandes de ticket |
| Épingler des messages | En-tête des tickets (`pinThreadHeader`) |
| Gérer les rôles | Rôle d'absence (le rôle du bot doit être **au-dessus** du rôle d'absence) |
| Gérer les salons | Salons d'essai des recrues, salons de tickets |
| Mentionner @everyone, @here et tous les rôles | Ping des rôles non mentionnables (transactions, tickets) |
| Ajouter des réactions | Option `reactOnSeen` des tickets |

Ces permissions sont données au rôle créé automatiquement pour le bot à l'invitation : rien d'autre à
configurer, sauf deux points :

- **Ordre des rôles** : dans *Paramètres du serveur → Rôles*, placer le rôle du bot **au-dessus** du rôle
  d'absence, sinon il ne peut pas le poser.
- **Catégories privées** (salons d'essai des recrues, tickets) : si `@everyone` n'y a pas accès, ajouter le
  rôle du bot à la catégorie avec *Voir le salon*, *Envoyer des messages*, *Gérer les salons* et
  *Gérer les permissions*. Les salons d'essai donnent ensuite eux-mêmes l'accès au bot.

### Récupérer les identifiants

Plusieurs réglages demandent des identifiants (serveur, salon, rôle). Activer **Paramètres utilisateur →
Avancés → Mode développeur**, puis clic droit sur un serveur, un salon ou un rôle → **Copier l'identifiant**.

---

## 2. Lancer le bot

Au démarrage, le bot crée ou met à jour la base de données, déploie les slash commands puis se connecte.

### En local

Prérequis : [Rust](https://rustup.rs) stable.

```bash
cp .env.example .env      # puis renseigner DISCORD_TOKEN et DISCORD_GUILD_IDS
cargo run --release
```

### Avec Docker

```bash
docker build -t bot-golmon .
docker run -d --name bot-golmon --restart unless-stopped \
  --env-file .env \
  -v golmon-data:/app/data \
  -p 8890:8890 \
  bot-golmon
```

`-p 8890:8890` ne sert qu'au module ticket.

### Déploiement sur Northflank

1. **Create new → Service → Combined service** (build et déploiement dans le même service).
2. **Repository** : ce dépôt et la branche à déployer. **Build type** : *Dockerfile*, chemin `/Dockerfile`,
   contexte `/`. Le build compile la feuille de style, puis le binaire Rust, puis produit une image
   Debian minimale (~20 Mo de binaire).
3. **Environment variables** : ajouter au minimum `DISCORD_TOKEN` et `DISCORD_GUILD_IDS` (voir
   [section 3](#3-variables-denvironnement)). Placer le token dans un *Secret group* plutôt qu'en clair.
4. **Volume** (Northflank → *Volumes → Add volume*, quelques centaines de Mo suffisent), monté sur
   **`/app/data`**. Il contient toutes les données :

   | Fichier | Contenu |
   |---|---|
   | `golmon.db` | Base principale |
   | `modmail.sqlite` | Base des tickets, si le module ticket est utilisé |
   | `logs/`, `attachments/` | Seulement avec `logStorage = attachment` ou `attachmentStorage = local` |

   Volume existant de l'ancienne image (monté sur `/app/prisma`) : changer seulement son chemin de
   montage en `/app/data`. Au démarrage, `dev.db` est renommée en `golmon.db`, sans perte.

   Un volume Northflank limite le service à **une instance** : c'est voulu, un bot Discord ne doit
   tourner qu'en un seul exemplaire.
5. **Ports** (module ticket uniquement) : port `8890`, protocole HTTP, **public**. Copier l'URL publique
   attribuée et la mettre dans `MM_URL`, sans `/` final. *Health check* possible sur le chemin `/healthz`.
6. **Resources** : le plus petit plan suffit (quelques dizaines de Mo de mémoire en fonctionnement).
7. Déployer, puis vérifier dans **Logs** la présence de `connecté à Discord` et
   `commandes déployées sur la guilde …`.

Pour mettre à jour : pousser sur la branche suivie, Northflank reconstruit et redéploie. Les migrations
de base s'appliquent au démarrage suivant.

---

## 3. Variables d'environnement

Copier `.env.example` en `.env` (local) ou les saisir dans Northflank.

| Variable | Défaut | Description |
|---|---|---|
| `DISCORD_TOKEN` | — | **Requis.** Token du bot. |
| `DISCORD_GUILD_IDS` | *(vide)* | Serveurs autorisés, séparés par des virgules (`111,222`). Le bot quitte automatiquement tout autre serveur, et les commandes sont déployées par serveur (disponibles immédiatement). Vide : aucune restriction, commandes globales (jusqu'à une heure de propagation). **À renseigner en production.** |
| `DATABASE_URL` | `file:./data/golmon.db` | Fichier SQLite principal. |
| `DEPLOY_COMMANDS` | `true` | Déploie les slash commands à chaque démarrage. `false` pour désactiver ; `bot-golmon deploy-commands` les déploie puis s'arrête. |
| `RUST_LOG` | `info` | Niveau de détail des logs : `error`, `warn`, `info`, `debug`. Par module : `info,bot_golmon::modmail=debug`. |
| `TZ` | `Europe/Paris` (image Docker) | Sans effet sur les dates du bot, toujours interprétées en heure de Paris. |
| `MM_*`, `MODMAIL_CONFIG`, `PORT` | — | Module ticket, voir [section 5](#5-tickets-modmail). |

---

## 4. Configurer les modules sur Discord

Chaque module se configure **une seule fois par serveur**, avec sa sous-commande `config`. Lancée sans
option, elle affiche la configuration actuelle. Modifier la configuration demande la permission **Gérer
le serveur** (pour présences, transactions et véhicules, la consulter aussi ; celle des recrues et des
numéros est lisible par tous). Les heures saisies sont toujours interprétées en **heure de Paris** (heure d'été comprise) ;
les dates affichées utilisent les timestamps Discord, chacun les voit dans son fuseau.

### Présences

```
/presence config role:@Membres absence_role:@Absent channel:#présences reminder_channel:#rappels absence_channel:#absences
```

| Option | Rôle |
|---|---|
| `role` | **Requis.** Membres concernés par les présences (pingés, comptés dans l'effectif). |
| `absence_role` | Rôle donné pendant une absence déclarée ; ses porteurs sont absents d'office et jamais pingés. |
| `channel` | Salon où sont publiées les présences (défaut : salon de la commande). |
| `reminder_channel` | Salon des rappels automatiques (défaut : salon de la présence). |
| `absence_channel` | Salon où sont annoncées les absences déclarées. |

Utilisation :

- `/presence create heure:21:00 [titre] [date:JJ/MM/AAAA] [description]` : la date vaut aujourd'hui par
  défaut, ou demain si l'heure est déjà passée. Le rôle est pingé, sauf les absents et la blocklist.
- Boutons **Présent / Absent / Retard** (durée demandée) **/ Ping**. L'embed liste les présents, absents,
  retards (🔄 = a changé d'avis) et les membres **sans réponse**.
- Automatique, toutes les 5 minutes : clôture à l'heure de l'événement ; rappel des non-répondants au
  plus toutes les 3 h ; pose et retrait du rôle d'absence.
- `/presence ping`, `close`, `changes` (historique des changements d'avis), `stats [joueur]` (fiche ou
  classement), `forcevote` (staff : saisir la réponse d'un membre, y compris sur une présence close 🔒),
  `cancel` (supprime une présence), `reset` (efface toutes les statistiques).
- **Blocklist** : `/presence blocklist add membre raison` : jamais pingé, absent d'office. `remove`, `list`.

### Absences

```
/absence declare motif:"Vacances" fin:20/08/2026 [debut:JJ/MM/AAAA] [membre:@…]
```

Une date couvre la journée entière. Le rôle d'absence est posé dès le début de la période et retiré à la
fin (ou à la fin de la dernière absence si plusieurs se chevauchent). Les présences ouvertes de la
période passent en **Absent**. `/absence end [membre]` termine une absence, `/absence list` les liste.
Déclarer ou clore l'absence d'un autre membre demande **Gérer les messages**.

> Si le rôle ne peut pas être posé (permission « Gérer les rôles » absente, ou rôle placé au-dessus de
> celui du bot), l'absence est quand même enregistrée et le problème est signalé.

### Transactions

```
/transaction config dashboard_channel:#transactions ping_role:@Logistique
```

- **Tableau de bord** permanent, mis à jour automatiquement : transactions en cours par statut, total
  engagé, dernières terminées. `clear_ping_role:true` retire le rôle mentionné à la création.
- `/transaction create groupe type:Vente prix:1500` publie la fiche dans le salon courant, avec boutons
  de statut, d'ajout d'objet et de suppression.
- `add-item`, `status`, `view`, `cancel` sont ouverts à tous ; `delete` demande **Gérer les messages**.
  Une transaction **clôturée** n'est plus modifiable.

### Véhicules

```
/vehicle config dashboard_channel:#véhicules
/vehicle add plaque:D6CS3P0G proprietaire:@Membre-ou-@Rôle [modele]
```

Tableau de bord groupé par propriétaire. Le modèle est récupéré via l'API GLife si non précisé (un échec
de l'API n'empêche pas l'ajout). `/vehicle lookup plaque` interroge l'API pour n'importe quelle plaque
(modèle, propriétaire **en jeu**, acheteur, illégal). `owner` transfère, `remove` retire. Les plaques sont
normalisées : `d6cs 3p0g` = `D6CS3P0G`.

### Recrues

```
/recrue config role_recrue:@Recrue role_grade:@Gradé role_lead:@Lead categorie:#Essais salon:#avis-recrues
```

| Option | Rôle |
|---|---|
| `role_recrue` | **Requis.** Rôle des membres en période d'essai : son attribution ajoute le membre au suivi. |
| `role_grade` | **Requis.** Rôle autorisé à donner son avis. |
| `role_lead` | Rôle autorisé à poser un véto (les leads votent aussi). |
| `categorie` | Catégorie où créer les salons d'essai privés. |
| `salon` | Salon où est publié le message d'avis de la semaine. |

- **Chaque semaine** (lundi 00:00), un nouveau message liste les recrues avec le décompte ✅ / ❌ / ⬜.
  Trois menus : donner son avis (vote **et motif obligatoires**, un avis par gradé, modifiable), poser
  un véto, voir le détail. Les semaines passées restent visibles et consultables via `/recrue historique`.
- Recevoir le rôle recrue crée un salon `essai-<pseudo>` visible par la recrue, les gradés et les leads.
  Perdre le rôle retire de la liste mais conserve le salon.
- `/recrue liste`, `avis`, `ajouter`, `retirer`, `salon`, `historique` (gradés) ; `veto poser` / `veto lever`
  (leads uniquement : « Gérer le serveur » ne donne **pas** le véto) ; `reset` force une nouvelle semaine.

### Numéros

```
/numero config salon:#annuaire
```

Annuaire permanent avec boutons « Ajouter / modifier mon numéro » et « Retirer mon numéro ». Format :
`555` + 4 à 6 chiffres (`555-1234` accepté). Un numéro par membre, unique sur le serveur.
`/numero ajouter numero [membre]` (pour un autre membre : **Gérer le serveur**), `voir`, `supprimer`, `liste`.

---

## 5. Tickets (modmail)

Les membres écrivent au bot **en message privé** ; un salon de ticket s'ouvre sur le serveur *inbox* et
le staff y répond avec des commandes texte. Le module reprend le comportement de modmailbot : même
configuration, mêmes commandes, même base de données. Une installation modmailbot existante peut être
remplacée sans rien changer.

### Mise en place

1. Choisir le **serveur inbox** (celui des membres ou un serveur staff séparé) et y créer :
   - une **catégorie** pour les tickets, visible uniquement du staff ;
   - un **salon de logs** (notifications de fermeture, mentions du bot, blocages expirés).
2. Activer le **Message Content Intent** ([section 1](#application-et-token)).
3. Configurer, au choix, par variables `MM_*` (pratique sur Northflank) ou par un fichier `config.ini`
   à la racine (`MODMAIL_CONFIG=/chemin/config.ini` pour un autre emplacement) :

   ```bash
   MM_MAIN_SERVER_ID=111111111111111111      # serveur(s) des membres ; plusieurs : 111||222
   MM_INBOX_SERVER_ID=222222222222222222     # serveur où s'ouvrent les tickets
   MM_LOG_CHANNEL_ID=333333333333333333      # salon de logs
   MM_NEW_THREAD_CATEGORY_ID=444444444444444444
   MM_MENTION_ROLE=555555555555555555        # rôle pingé à chaque nouveau ticket (ou here / none)
   MM_INBOX_SERVER_PERMISSION=555555555555555555   # qui est staff (rôle, utilisateur ou permission)
   MM_URL=https://ticket-xxxx.code.run       # URL publique du port 8890
   MM_RESPONSE_MESSAGE=Merci pour ton message ! Le staff te répond ici dès que possible.
   MM_CLOSE_MESSAGE=Ce ticket est fermé. Écris-nous à nouveau si besoin.
   ```

   Équivalent `config.ini` (voir `config.example.ini`) :

   ```ini
   mainServerId = 111111111111111111
   inboxServerId = 222222222222222222
   logChannelId = 333333333333333333
   newThreadCategoryId = 444444444444444444
   mentionRole = 555555555555555555
   inboxServerPermission = 555555555555555555
   url = https://ticket-xxxx.code.run
   responseMessage = Merci pour ton message ! Le staff te répond ici dès que possible.
   closeMessage = Ce ticket est fermé. Écris-nous à nouveau si besoin.
   ```

4. Exposer le **port 8890** (`port`/`MM_PORT`, ou `PORT`) et monter `/app/data` sur un volume
   ([Northflank](#déploiement-sur-northflank)).

Le module est **désactivé** tant qu'il n'y a ni `config.ini` ni variable `MM_*` (hors `MM_TOKEN`). Le bot
utilise son propre token (`DISCORD_TOKEN`) : l'option `token` de modmailbot est ignorée.

**Conversion des noms** : `MM_` + nom de l'option en majuscules, mots séparés par `_`
(`closeMessage` → `MM_CLOSE_MESSAGE`). Une option imbriquée s'écrit avec `__`
(`commandAliases.mv` → `MM_COMMAND_ALIASES__MV`), une liste avec `||` (`MM_MENTION_ROLE=111||222`). Dans
`config.ini`, une liste s'écrit `mentionRole[] = 111` sur plusieurs lignes, un texte multiligne
`responseMessage[] = ligne`.

### Options

Toutes les options de modmailbot sont reconnues, avec les mêmes valeurs par défaut. Les booléens
acceptent `true/false`, `on/off`, `yes/no`, `1/0`. Les durées s'écrivent `1h30m`, `2d`, `15m`.

<details>
<summary><b>Serveurs, accès et apparence</b></summary>

| Option | Défaut | Description |
|---|---|---|
| `mainServerId` | — | **Requis.** Serveur(s) des membres. |
| `inboxServerId` | — | **Requis.** Serveur des salons de tickets. |
| `logChannelId` | — | **Requis.** Salon de logs. |
| `prefix` | `!` | Préfixe des commandes. |
| `inboxServerPermission` | `manageMessages` | Qui est staff : noms de permissions (`manageMessages`, `kickMembers`, `administrator`…), identifiants de rôles ou d'utilisateurs. Le propriétaire du serveur l'est toujours. |
| `status` / `statusType` / `statusUrl` | `Message me for help!` / `playing` | Statut du bot (`playing`, `watching`, `listening`, `competing`, `custom`, `streaming` + URL Twitch). `none` pour aucun. |
| `useDisplaynames` | `true` | Afficher le nom global plutôt que le nom d'utilisateur. |
| `useNicknames` | `false` | Signer les réponses avec le surnom du serveur. |
| `breakFormattingForNames` | `true` | Échapper le markdown dans les noms. |
| `fallbackRoleName` | — | Rôle affiché si le modérateur n'a aucun rôle « affiché séparément ». |
| `overrideRoleNameDisplay` | — | Remplace le rôle affiché dans toutes les réponses. |
| `allowChangingDisplayRole` | `true` | Autorise `!role` pour choisir son rôle affiché. |

</details>

<details>
<summary><b>Ouverture des tickets</b></summary>

| Option | Défaut | Description |
|---|---|---|
| `newThreadCategoryId` | — | Catégorie des nouveaux tickets. |
| `categoryAutomation.newThreadFromServer.<idServeur>` | — | Catégorie selon le serveur d'origine (plusieurs serveurs principaux). |
| `mentionRole` | `none` | Mention à chaque nouveau ticket : identifiant de rôle, `here`, `everyone` ou `none`. |
| `responseMessage` | message anglais | Réponse automatique au premier message. |
| `showResponseMessageInThreadChannel` | `true` | Afficher cette réponse dans le salon du ticket. |
| `ignoreAccidentalThreads` | `false` | « ok », « merci »… n'ouvrent pas de ticket. |
| `requiredAccountAge` / `accountAgeDeniedMessage` | — | Âge minimum du compte, en heures. |
| `requiredTimeOnServer` / `timeOnServerDeniedMessage` | — | Ancienneté minimum sur le serveur, en minutes. |
| `anonymizeChannelName` | `false` | Nom de salon aléatoire plutôt que le pseudo. |
| `mentionUserInThreadHeader` / `rolesInThreadHeader` | `false` | Mention et rôles de l'utilisateur dans l'en-tête. |
| `pinThreadHeader` | `false` | Épingler l'en-tête du ticket. |
| `createThreadOnMention` | `false` | Ouvrir un ticket quand un membre mentionne le bot. |
| `pingOnBotMention` / `botMentionResponse` | `true` / — | Alerter le staff (et répondre) quand le bot est mentionné. |

</details>

<details>
<summary><b>Réponses, fermeture et blocages</b></summary>

| Option | Défaut | Description |
|---|---|---|
| `alwaysReply` / `alwaysReplyAnon` | `false` | Tout message du staff dans un ticket est envoyé à l'utilisateur (anonymement). |
| `forceAnon` | `false` | `!reply` et les snippets sont toujours anonymes (`!realreply` reste signé). |
| `allowStaffEdit` / `allowStaffDelete` | `true` | Commandes `!edit` et `!delete`. |
| `relayInlineReplies` | `true` | Relayer les réponses en citation. |
| `autoAlert` / `autoAlertDelay` | `false` / `2m` | Alerte automatique du modérateur après sa réponse. |
| `closeMessage` | — | Message envoyé à l'utilisateur à la fermeture. |
| `allowUserClose` | `false` | L'utilisateur peut fermer avec `!close` en message privé. |
| `allowSuspend` | `true` | Commandes `!suspend` / `!unsuspend`. |
| `allowMove` / `syncPermissionsOnMove` | `false` / `true` | Commande `!move` (et alignement des permissions sur la catégorie). |
| `allowBlock` | `true` | Commandes de blocage. |
| `blockMessage`, `timedBlockMessage`, `unblockMessage`, `timedUnblockMessage` | — | Messages envoyés à l'utilisateur (`{duration}`, `{delay}`, `{timestamp}`). |
| `blockedReply` | — | Réponse aux messages d'un utilisateur bloqué. |
| `allowNotes` | `true` | Notes sur les utilisateurs. |
| `commandAliases.<alias>` | — | Alias personnalisés, ex. `commandAliases.mv = move`. |

</details>

<details>
<summary><b>Snippets, notifications, pièces jointes et logs</b></summary>

| Option | Défaut | Description |
|---|---|---|
| `allowSnippets` | `true` | Réponses prédéfinies. |
| `snippetPrefix` / `snippetPrefixAnon` | `!!` / `!!!` | Utilisation d'un snippet (signé / anonyme). |
| `allowInlineSnippets`, `inlineSnippetStart`, `inlineSnippetEnd` | `true`, `{{`, `}}` | Snippets dans une réponse : `{{regles}}`. |
| `errorOnUnknownInlineSnippet` | `true` | Bloquer la réponse si un snippet n'existe pas. |
| `reactOnSeen` / `reactOnSeenEmoji` | `false` / 📨 | Réagir aux messages reçus. |
| `notifyOnReaction` / `notifyOnReactionRemoval` | `false` | Signaler les réactions de l'utilisateur aux réponses. |
| `notifyOnMainServerJoin` / `notifyOnMainServerLeave` | `true` | Signaler arrivées, départs et bannissements. |
| `typingProxy` / `typingProxyReverse` | `false` | Relayer l'indicateur « en train d'écrire ». |
| `updateMessagesLive` | `false` | Répercuter les éditions et suppressions de l'utilisateur. |
| `threadTimestamps` | `false` | Heure devant chaque message du ticket. |
| `enableGreeting`, `greetingMessage`, `greetingAttachment`, `serverGreetings.<id>.*` | `false` | Message privé de bienvenue aux nouveaux membres. |
| `attachmentStorage` | `original` | `original` (liens Discord), `local` (copie servie par le bot), `discord` (copie dans un salon). |
| `attachmentStorageChannelId` | — | Salon de stockage pour `discord`. |
| `relaySmallAttachmentsAsAttachments` / `smallAttachmentLimit` | `false` / 2 Mo | Renvoyer les petits fichiers en pièces jointes. |
| `logStorage` | `local` | `local` (page web), `attachment` (fichier .txt joint à la fermeture), `none`. |
| `logOptions.attachmentDirectory` | `data/logs` | Dossier des logs `attachment`. |
| `url` | — | URL publique du serveur web, utilisée dans les liens. **À définir en production.** |
| `port` / `host` | `8890` / `0.0.0.0` | Écoute du serveur web (`PORT` est aussi accepté). |
| `sqliteOptions.filename` | `data/modmail.sqlite` | Base des tickets. |

</details>

Non repris de modmailbot : les **plugins JavaScript**, les **notifications de nouvelle version** et
`dbType = mysql` (SQLite uniquement).

### Commandes du staff

Dans le salon d'un ticket :

| Commande | Effet |
|---|---|
| `!reply <texte>` · `!r` | Répondre (pièces jointes acceptées). |
| `!anonreply <texte>` · `!ar` | Répondre anonymement (seul le rôle est affiché). |
| `!realreply <texte>` · `!rr` | Répondre signé, même avec `forceAnon`. |
| `!edit <n°> <texte>` · `!e` / `!delete <n°>` · `!d` | Modifier ou supprimer sa réponse n° (numéro affiché devant la réponse). |
| `!close` | Fermer. `!close 1h` : fermeture programmée, annulée par un nouveau message ; `-s` : sans prévenir l'utilisateur ; `!close cancel`. |
| `!suspend [délai]` / `!suspend cancel` / `!unsuspend` | Mettre le ticket en pause (agit comme fermé). |
| `!alert` / `!alert cancel` | Être pingé à la prochaine réponse de l'utilisateur. |
| `!move <catégorie>` | Déplacer le ticket (avec `allowMove`). |
| `!logs [page]`, `!loglink [-s] [-v]` | Anciens tickets de l'utilisateur, lien du log de ce ticket. |
| `!block [durée]` / `!unblock [délai]` | Bloquer ou débloquer l'utilisateur. |
| `!note <texte>` / `!notes` / `!delete_note <id>` | Notes internes sur l'utilisateur. |
| `!role`, `!role <nom>`, `!role reset` | Rôle affiché dans ses réponses pour ce ticket. |
| `!id`, `!dm_channel_id`, `!message <n°>` | Identifiants utiles (signalements à Discord). |
| `!!snippet` / `!!!snippet` | Envoyer un snippet (signé / anonyme), `{1}`, `{2}`… remplacés par les arguments. |

Partout sur le serveur inbox : `!newthread <id>`, `!logs <id> [page]`, `!log <n° ou id>`,
`!block <id> [durée]`, `!unblock <id>`, `!is_blocked <id>`, `!blocklist`, `!notes <id>`,
`!note <id> <texte>`, `!role` / `!role <nom>` / `!role reset` (rôle par défaut), `!version`, et pour les snippets
`!snippet <nom> [texte]` · `!s`, `!edit_snippet` · `!es`, `!delete_snippet` · `!ds`, `!snippets`.

Tout message du staff qui n'est pas une commande reste **interne** (note visible du staff seulement),
sauf avec `alwaysReply`.

### Logs de tickets

À la fermeture, le salon de logs reçoit un résumé et le lien `MM_URL/logs/<id>`. La page affiche le ticket
de façon lisible (thème clair ou sombre selon le système, adaptée au mobile) :

- messages de l'utilisateur, réponses du staff (numéro, rôle, anonymat), notes internes, commandes ;
- réponses modifiées (avant / après) et supprimées, images en aperçu, fichiers, embeds, markdown Discord ;
- filtres **Échanges seulement** (`?simple=1`) et **Détails techniques** (`?verbose=1`) ;
- **Texte brut** (`?raw=1`) : log au format exact de modmailbot.

Les liens contiennent l'identifiant unique du ticket, non devinable, mais toute personne disposant du
lien peut lire le log.

### Reprendre une installation modmailbot

1. Arrêter modmailbot.
2. Copier son `db/data.sqlite` en `modmail.sqlite` sur le volume `/app/data` (et `logs/`, `attachments/` le cas échéant).
3. Reprendre son `config.ini` à la racine, ou convertir ses options en variables `MM_*`.
4. Démarrer le bot : la base est reconnue telle quelle (mêmes migrations `knex_migrations`) ; tickets
   ouverts, historique, snippets, notes et blocages sont conservés, et les messages reçus pendant
   l'arrêt sont récupérés.

---

## 6. Mise à jour depuis la version TypeScript

Seule action : changer le chemin de montage du volume existant de `/app/prisma` en `/app/data`, en gardant les mêmes variables :

- au premier démarrage, `dev.db` est renommée en `golmon.db` (si le volume est encore monté sur
  `/app/prisma`, la base y est quand même utilisée, avec un avertissement dans les logs) ;
- la base est mise à jour **sans perte** : schéma repris, tables `Prospect*`
  renommées en `Recrue*` (avis, vétos, salons d'essai conservés), dates normalisées ;
- `/prospect` disparaît, remplacée par `/recrue`. Options renommées : `role_prospect` → `role_recrue` ;
- les messages déjà publiés (présences, tableaux de bord, menus de la semaine) continuent de fonctionner ;
- l'ancien script `pnpm deploy-commands` est inutile : les commandes sont déployées au démarrage.

---

## 7. Dépannage

| Symptôme | Cause probable |
|---|---|
| Le bot démarre puis s'arrête avec « DISCORD_TOKEN manquant » ou une erreur 401 | Token absent ou régénéré : mettre à jour `DISCORD_TOKEN`. |
| Erreur `Disallowed intents` au démarrage | Intent privilégié non activé dans le portail ([section 1](#application-et-token)). |
| Les slash commands n'apparaissent pas | Bot invité sans le scope `applications.commands`, ou `DISCORD_GUILD_IDS` vide (commandes globales : jusqu'à une heure). Recharger Discord (Ctrl+R). |
| Le bot quitte un serveur dès qu'il le rejoint | Serveur absent de `DISCORD_GUILD_IDS`. |
| « Rôle d'absence non appliqué » | Permission « Gérer les rôles » manquante, ou rôle d'absence au-dessus du rôle du bot. |
| Effectif ou ping des présences vide | *Server Members Intent* désactivé. |
| « Création du salon impossible » (recrues) | Permission « Gérer les salons » manquante, catégorie supprimée ou pleine (50 salons). |
| Le bot ne réagit pas aux messages privés ni aux `!commandes` | Module ticket non configuré (`module ticket désactivé` dans les logs) ou *Message Content Intent* désactivé. |
| `!reply` ne fait rien | L'auteur n'est pas reconnu comme staff : vérifier `inboxServerPermission`. |
| Les liens de logs pointent vers une IP injoignable | `MM_URL` non défini, ou port 8890 non exposé publiquement. |
| Tableau de bord figé | Message supprimé ou salon inaccessible : relancer la commande `config` du module. |

Pour plus de détails : `RUST_LOG=debug`.

---

## 8. Développement

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

La CI (`.github/workflows/ci.yml`) vérifie le formatage, clippy, les tests et le build Docker.

| Dossier | Contenu |
|---|---|
| `src/discord/` | Gateway, aiguillage des interactions, réponses, options, composants, cache des membres |
| `src/modules/` | Un dossier par module à slash commands (`commands`, `service`, `embed`, `interactions`, `cron`) |
| `src/modmail/` | Tickets : configuration, base knex, tickets, commandes texte, événements, serveur web |
| `src/util/` | Dates (Europe/Paris), mise en forme bornée aux limites Discord, identifiants |
| `migrations/` | Migrations SQL de la base principale (appliquées au démarrage) |
| `templates/`, `web/` | Page de logs (Askama) et feuille de style Tailwind CSS v4 |

**Thème des logs** : les couleurs sont des tokens sémantiques dans `web/src/logs.css` (valeurs sur
`:root`, redéfinies en sombre, exposées via `@theme inline` : `bg-surface`, `text-ink-muted`,
`border-l-staff`…). Après modification :

```bash
cd web && npm ci && npm run build   # régénère web/dist/logs.css, embarqué à la compilation
```

**Nouvelle colonne en base** : ajouter un fichier `migrations/000N_description.sql` (ne jamais modifier
une migration déjà déployée).
