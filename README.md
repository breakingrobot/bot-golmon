# Bot Discord

Bot Discord pour un groupe RP : **présences et absences**, **transactions**, **véhicules**, **recrues**,
**annuaire téléphonique** et **tickets** (modmail compatible [Dragory/modmailbot](https://github.com/Dragory/modmailbot)).

Écrit en Rust ([Twilight](https://twilight.rs)) avec SQLite, livré sous forme d'image Docker.

## Démarrage rapide

1. **Créer le bot** sur le portail Discord et l'inviter : [docs/discord.md](docs/discord.md).
2. **Déployer** sur Northflank (ou avec Docker) avec un volume sur `/app/data` et deux variables,
   `DISCORD_TOKEN` et `DISCORD_GUILD_IDS` : [docs/deploiement.md](docs/deploiement.md).
3. **Configurer les modules** sur Discord (`/presence config`, `/recrue config`…) : [docs/modules.md](docs/modules.md).

Le bot crée sa base et déploie ses slash commands tout seul au démarrage.

## Documentation

| Page | Contenu |
|---|---|
| [Discord](docs/discord.md) | Application, token, intents, invitation et permissions |
| [Déploiement](docs/deploiement.md) | Northflank, site web, HTTPS, connexion Discord, Docker, variables |
| [Modules](docs/modules.md) | Présences, absences, transactions, véhicules, recrues, numéros |
| [Tickets](docs/tickets.md) | Module modmail : mise en place, boutons, commandes, site web des logs |
| [Options des tickets](docs/tickets-options.md) | Toutes les options modmailbot reconnues |
| [Dépannage](docs/depannage.md) | Problèmes fréquents |
| [Développement](docs/developpement.md) | Compiler, tester, structure du code |
