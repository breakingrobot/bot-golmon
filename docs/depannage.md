# Dépannage

| Symptôme | Cause probable |
|---|---|
| « DISCORD_TOKEN manquant » ou erreur 401 | Token absent ou régénéré : mettre à jour `DISCORD_TOKEN`. |
| `Disallowed intents` au démarrage | Intent privilégié non activé ([Discord](discord.md#application-et-token)). |
| Les slash commands n'apparaissent pas | Invitation sans `applications.commands`, ou `DISCORD_GUILD_IDS` vide (jusqu'à une heure). Recharger Discord (Ctrl+R). |
| Le bot quitte un serveur dès qu'il le rejoint | Serveur absent de `DISCORD_GUILD_IDS`. |
| « Rôle d'absence non appliqué » | « Gérer les rôles » manquante, ou rôle d'absence au-dessus de celui du bot. |
| Effectif ou ping des présences vide | *Server Members Intent* désactivé. |
| « Création du salon impossible » (recrues) | « Gérer les salons » manquante, catégorie supprimée ou pleine (50 salons). |
| Aucune réaction aux messages privés ni aux `!commandes` | Module ticket non configuré (`module ticket désactivé` dans les logs) ou *Message Content Intent* désactivé. |
| `!reply` ne fait rien | Auteur non reconnu comme staff : vérifier `inboxServerPermission`. |
| Liens de logs injoignables | `MM_URL`/`WEB_URL` non défini ou port 8890 non public ([site web](deploiement.md#site-web-et-https)). |
| « Connexion non configurée » sur le site | `DISCORD_CLIENT_SECRET` ou `WEB_URL` manquant. |
| Discord affiche « redirect_uri invalide » | Redirection `<WEB_URL>/auth/callback` absente du portail (OAuth2 → Redirects), ou `WEB_URL` différent de l'adresse réelle. |
| « Accès refusé » après connexion | Compte hors du staff des tickets (`inboxServerPermission`) ou sans « Gérer le serveur ». |
| Boutons du ticket sans effet | Membre hors du staff, ou ticket déjà fermé. |
| Tableau de bord figé | Message supprimé ou salon inaccessible : relancer la commande `config` du module. |

Plus de détails dans les logs avec `RUST_LOG=debug`.
