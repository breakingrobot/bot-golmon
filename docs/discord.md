# Créer le bot sur Discord

## Application et token

1. [Portail développeur Discord](https://discord.com/developers/applications) → **New Application**.
2. Onglet **Bot** → **Reset Token** : le token est la valeur de `DISCORD_TOKEN`. Ne jamais le partager ;
   s'il fuite, le régénérer.
3. Toujours dans **Bot**, section *Privileged Gateway Intents* :
   - **Server Members Intent** : toujours ;
   - **Message Content Intent** : avec le [module ticket](tickets.md) ou le module [coffres](modules.md#coffres)
     (lecture des messages du jeu).
4. Onglet **OAuth2** : *Client Secret* et redirection du site web, voir
   [Site web → Connexion Discord](site-web.md#2-connexion-discord).

## Inviter le bot

Remplacer `CLIENT_ID` par l'*Application ID* (onglet **General Information**) :

```
https://discord.com/oauth2/authorize?client_id=CLIENT_ID&scope=bot+applications.commands&permissions=2252092140153936
```

Le rôle créé pour le bot reçoit ces permissions :

| Permission | Pour |
|---|---|
| Voir les salons, Envoyer des messages, Intégrer des liens, Joindre des fichiers, Voir l'historique | Tous les modules |
| Gérer les messages | Suppression des messages de transaction et des commandes de ticket |
| Épingler des messages | En-tête des tickets (`pinThreadHeader`) |
| Gérer les rôles | Rôle d'absence |
| Gérer les salons | Salons d'essai des recrues, salons de tickets |
| Gérer les fils, Envoyer des messages dans les fils | Tickets en fils de forum (`newThreadForumId`) |
| Mentionner @everyone, @here et tous les rôles | Ping des rôles non mentionnables |
| Ajouter des réactions | Option `reactOnSeen` des tickets |

Deux réglages restent à faire à la main :

- **Ordre des rôles** (*Paramètres du serveur → Rôles*) : le rôle du bot doit être **au-dessus** du rôle
  d'absence.
- **Catégories privées** (essais des recrues, tickets) : si `@everyone` ne les voit pas, y ajouter le rôle
  du bot avec *Voir le salon*, *Envoyer des messages*, *Gérer les salons* et *Gérer les permissions*.
  Pour un **forum de tickets**, même chose sur le forum, avec *Gérer les fils* et *Envoyer des messages
  dans les fils*.

## Copier un identifiant

**Paramètres utilisateur → Avancés → Mode développeur**, puis clic droit sur un serveur, un salon ou un
rôle → **Copier l'identifiant**.
