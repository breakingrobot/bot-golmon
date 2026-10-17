# Tickets (modmail)

Les membres écrivent au bot **en message privé** ; un salon de ticket s'ouvre sur le serveur *inbox* et le
staff y répond avec des commandes texte. Même configuration, mêmes commandes et même base que
[modmailbot](https://github.com/Dragory/modmailbot).

Le module est **désactivé** tant qu'il n'y a ni `config.ini` ni variable `MM_*`.

## Mise en place

1. Sur le **serveur inbox** (celui des membres ou un serveur staff), créer une **catégorie** visible du
   staff seulement et un **salon de logs**.
2. Activer le **Message Content Intent** ([Discord](discord.md#application-et-token)).
3. Ajouter les variables (ou un `config.ini` à la racine, voir `config.example.ini`) :

   ```bash
   MM_MAIN_SERVER_ID=111111111111111111          # serveur des membres
   MM_INBOX_SERVER_ID=222222222222222222         # serveur des tickets
   MM_LOG_CHANNEL_ID=333333333333333333          # salon de logs
   MM_NEW_THREAD_CATEGORY_ID=444444444444444444  # catégorie des tickets
   MM_MENTION_ROLE=555555555555555555            # rôle pingé à chaque ticket (ou here / none)
   MM_INBOX_SERVER_PERMISSION=555555555555555555 # qui est staff (rôle, utilisateur ou permission)
   MM_URL=https://tickets.mon-domaine.fr         # adresse HTTPS des logs
   MM_RESPONSE_MESSAGE=Merci ! Le staff te répond ici dès que possible.
   MM_CLOSE_MESSAGE=Ce ticket est fermé. Écris-nous à nouveau si besoin.
   ```

4. Exposer le port `8890` en HTTPS : [Déploiement → HTTPS](deploiement.md#https-module-ticket).

**Nom des variables** : `MM_` + option en majuscules, mots séparés par `_` (`closeMessage` →
`MM_CLOSE_MESSAGE`). Option imbriquée : `__` (`commandAliases.mv` → `MM_COMMAND_ALIASES__MV`). Liste :
`||` (`MM_MENTION_ROLE=111||222`).

Toutes les options : [Options des tickets](tickets-options.md).

## Commandes du staff

Dans un salon de ticket :

| Commande | Effet |
|---|---|
| `!r <texte>` · `!ar <texte>` | Répondre, signé ou anonyme (pièces jointes acceptées). |
| `!e <n°> <texte>` · `!d <n°>` | Modifier ou supprimer sa réponse n°. |
| `!close` | Fermer. `!close 1h` : plus tard ; `-s` : sans prévenir ; `!close cancel`. |
| `!suspend` / `!unsuspend` | Mettre en pause. |
| `!alert` | Être pingé à la prochaine réponse de l'utilisateur. |
| `!move <catégorie>` | Déplacer (avec `allowMove`). |
| `!logs`, `!loglink` | Anciens tickets de l'utilisateur, lien du log. |
| `!block [durée]` / `!unblock` | Bloquer ou débloquer. |
| `!note <texte>` / `!notes` | Notes internes sur l'utilisateur. |
| `!role [nom]` | Rôle affiché dans ses réponses. |
| `!!snippet` / `!!!snippet` | Réponse prédéfinie, signée ou anonyme. |

Partout sur le serveur inbox : `!newthread <id>`, `!logs <id>`, `!block <id>`, `!unblock <id>`,
`!blocklist`, `!notes <id>`, et la gestion des snippets (`!snippet <nom> [texte]`, `!snippets`,
`!edit_snippet`, `!delete_snippet`).

Un message du staff qui n'est pas une commande reste **interne**, sauf avec `alwaysReply`.

## Logs

À la fermeture, le salon de logs reçoit le lien `MM_URL/logs/<id>` : une page lisible (clair/sombre,
mobile) avec les échanges, notes internes, modifications et pièces jointes. Filtres **Échanges seulement**
et **Détails techniques**, et **Texte brut** au format modmailbot.

Le lien n'est pas devinable, mais toute personne qui l'a peut lire le log.

## Reprendre une installation modmailbot

1. Arrêter modmailbot.
2. Copier `db/data.sqlite` dans le volume `/app/data` sous le nom `modmail.sqlite` (et `logs/`,
   `attachments/` le cas échéant).
3. Reprendre son `config.ini` ou convertir ses options en variables `MM_*`.
4. Démarrer : tickets ouverts, historique, snippets, notes et blocages sont conservés.

Le bot utilise `DISCORD_TOKEN` ; l'option `token` de modmailbot est ignorée. Non repris : plugins
JavaScript et MySQL.
