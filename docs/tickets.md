# Tickets (modmail)

Les membres écrivent au bot **en message privé** ; un salon de ticket s'ouvre sur le serveur *inbox* et le
staff y répond. Le module reprend [modmailbot](https://github.com/Dragory/modmailbot) : mêmes options,
mêmes commandes, même base de données. Une installation modmailbot peut être remplacée sans rien perdre.

Tous les messages du bot (aux utilisateurs, dans les tickets, dans le salon de logs) sont en français.

## Fonctionnement

1. Un membre écrit au bot en privé. Le bot ouvre un salon de ticket (ou un fil de forum), mentionne le
   rôle du staff et répond au membre (`responseMessage`).
2. L'en-tête du ticket présente le membre (âge du compte, pseudo, arrivée sur le serveur, anciens tickets,
   notes) avec des [boutons](#boutons-du-ticket).
3. Le staff répond avec les boutons ou `!r` ; les messages qui ne sont pas des commandes restent
   **internes** (visibles du staff seulement).
4. À la fermeture, le salon est supprimé (ou le fil archivé), le membre reçoit `closeMessage`, et le salon
   de logs reçoit le lien du [log](site-web.md#log-dun-ticket).

## Mise en place

1. Sur le **serveur inbox** (celui des membres, ou un serveur réservé au staff) :
   - une **catégorie** visible du staff seulement (ou un **salon forum**, voir plus bas) ;
   - un **salon de logs** (fermetures, mentions du bot, blocages expirés).
2. Activer le **Message Content Intent** ([Discord](discord.md#application-et-token)) et vérifier les
   permissions du bot sur la catégorie ([Discord](discord.md#inviter-le-bot)).
3. Ajouter les variables :

   ```bash
   MM_MAIN_SERVER_ID=111111111111111111          # serveur des membres (plusieurs : 111||222)
   MM_INBOX_SERVER_ID=111111111111111111         # serveur des tickets (peut être le même)
   MM_LOG_CHANNEL_ID=222222222222222222          # salon de logs
   MM_NEW_THREAD_CATEGORY_ID=333333333333333333  # catégorie des tickets
   MM_INBOX_SERVER_PERMISSION=444444444444444444 # qui est staff : rôle, utilisateur ou permission
   MM_MENTION_ROLE=444444444444444444            # rôle pingé à chaque ticket (ou here / none)
   MM_URL=https://bot.mon-domaine.fr             # adresse du site, pour les liens de logs
   MM_RESPONSE_MESSAGE=Merci ! Le staff te répond ici dès que possible.
   MM_CLOSE_MESSAGE=Ce ticket est fermé. Écris-nous à nouveau si besoin.
   ```

4. Mettre en place le [site web](site-web.md) (HTTPS et connexion Discord) pour lire les logs.

Le module est **désactivé** tant qu'aucune variable `MM_*` (hors `MM_TOKEN`) ni fichier de configuration
n'existe. Le bot utilise `DISCORD_TOKEN` ; l'option `token` de modmailbot est ignorée.

### Variables ou `config.ini`

Les options s'écrivent au choix en variables d'environnement (pratique sur Northflank) ou dans un
`config.ini` au format modmailbot (voir `config.example.ini`). Les variables ont priorité.

- **Variables** : `MM_` + nom de l'option en majuscules, mots séparés par `_` (`closeMessage` →
  `MM_CLOSE_MESSAGE`). Option imbriquée : `__` (`commandAliases.mv` → `MM_COMMAND_ALIASES__MV`). Liste :
  `||` (`MM_MENTION_ROLE=111||222`).
- **Fichier** : cherché dans le dossier courant (`/app` dans l'image Docker, qui n'est pas persistant).
  Dans Docker, le placer sur le volume et l'indiquer : `MODMAIL_CONFIG=/app/data/config.ini`.

Toutes les options : [Options des tickets](tickets-options.md).

### Fils de forum

Avec `MM_NEW_THREAD_FORUM_ID=<id d'un salon forum>` à la place de la catégorie, chaque ticket est une
publication du forum :

- plus de limite de 50 salons par catégorie ;
- à la fermeture, la publication est **archivée et verrouillée** au lieu d'être supprimée : l'historique
  reste consultable sur Discord ;
- `!move` n'est pas disponible ;
- le bot a besoin de *Gérer les fils* et *Envoyer des messages dans les fils* sur le forum.

## Boutons du ticket

Sous l'en-tête de chaque nouveau ticket :

| Bouton | Effet |
|---|---|
| 🙋 **Prendre en charge** | Attribue le ticket à la personne qui clique (nom affiché sur le bouton, dans la [liste web](site-web.md#tickets) et les statistiques). Un autre membre du staff peut le reprendre ; recliquer le libère. |
| ✉️ **Répondre** | Fenêtre de saisie ; la réponse part comme un `!r`. |
| **Répondre anonymement** | Idem, comme un `!ar` (seul le rôle est affiché). |
| 🔒 **Fermer** | Demande confirmation : fermer, ou fermer sans prévenir l'utilisateur. |

Seul le staff peut utiliser les boutons.

## Commandes du staff

Dans le salon d'un ticket :

| Commande | Effet |
|---|---|
| `!r <texte>` · `!ar <texte>` | Répondre, signé ou anonyme (pièces jointes acceptées). |
| `!rr <texte>` | Répondre signé, même avec `forceAnon`. |
| `!e <n°> <texte>` · `!d <n°>` | Modifier ou supprimer sa réponse n° (numéro affiché devant la réponse). |
| `!close` | Fermer. `!close 1h` : fermeture programmée (annulée si l'utilisateur répond) ; `-s` : sans prévenir ; `!close cancel`. |
| `!suspend [délai]` / `!unsuspend` | Mettre le ticket en pause (il se comporte comme fermé). |
| `!alert` / `!alert cancel` | Être pingé à la prochaine réponse de l'utilisateur. |
| `!move <catégorie>` | Déplacer le ticket (avec `allowMove`). |
| `!logs [page]` · `!loglink` | Anciens tickets de l'utilisateur, lien du log de ce ticket. |
| `!block [durée]` / `!unblock` | Bloquer ou débloquer l'utilisateur. |
| `!note <texte>` / `!notes` | Notes internes sur l'utilisateur. |
| `!role [nom]` · `!role reset` | Rôle affiché dans ses réponses pour ce ticket. |
| `!!snippet` / `!!!snippet` | Réponse prédéfinie, signée ou anonyme (`{1}`, `{2}`… remplacés par les arguments). |

Partout sur le serveur inbox : `!newthread <id>`, `!logs <id>`, `!block <id> [durée]`, `!unblock <id>`,
`!is_blocked <id>`, `!blocklist`, `!notes <id>`, `!note <id> <texte>`, `!version`, et les snippets
(`!snippet <nom> [texte]` · `!s`, `!edit_snippet` · `!es`, `!delete_snippet` · `!ds`, `!snippets`).

Les durées s'écrivent `15m`, `1h30m`, `2d`.

## Reprendre une installation modmailbot

1. Arrêter modmailbot.
2. Copier son `db/data.sqlite` dans le volume `/app/data` sous le nom `modmail.sqlite` (et `logs/`,
   `attachments/` le cas échéant).
3. Reprendre son `config.ini` (voir [plus haut](#variables-ou-configini)) ou convertir ses options en
   variables `MM_*`.
4. Démarrer : la base est reconnue telle quelle (mêmes migrations `knex_migrations`). Tickets ouverts,
   historique, snippets, notes et blocages sont conservés, et les messages reçus pendant l'arrêt sont
   récupérés.

Les prises en charge sont stockées dans la base principale du bot : `modmail.sqlite` garde le schéma exact
de modmailbot, un retour en arrière reste possible. Non repris de modmailbot : plugins JavaScript,
notifications de nouvelle version, MySQL.
