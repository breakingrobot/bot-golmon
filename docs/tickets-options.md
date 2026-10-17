# Options des tickets

Retour à la [mise en place des tickets](tickets.md).

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
