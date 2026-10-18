# Développement

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

La CI (`.github/workflows/ci.yml`) vérifie le formatage, clippy, les tests et le build Docker.

| Dossier | Contenu |
|---|---|
| `src/discord/` | Gateway, interactions, réponses, composants, cache des membres |
| `src/modules/` | Un dossier par module à slash commands |
| `src/modmail/` | Tickets : configuration, base, commandes texte, boutons, pages des logs |
| `src/web/` | Site web : serveur, connexion Discord, mise en page, statistiques, rendu du markdown Discord |
| `src/util/` | Dates (Europe/Paris), mise en forme, identifiants |
| `migrations/` | Migrations SQL de la base principale, appliquées au démarrage |
| `templates/`, `web/` | Pages du site (Askama, `base.html` commun) et feuille de style Tailwind CSS v4 |

**Nouvelle colonne en base** : ajouter `migrations/000N_description.sql`, ne jamais modifier une migration
déjà déployée.

**Thème du site** : les couleurs sont des tokens sémantiques dans `web/src/app.css` (exposés via
`@theme inline` : `bg-surface`, `text-ink-muted`…). Après modification :

```bash
cd web && npm ci && npm run build   # régénère web/dist/app.css, embarqué à la compilation
```

**Aperçu des pages** sans Discord : les tests de rendu écrivent une page d'exemple (feuille de style
incluse) si la variable correspondante est définie.

```bash
LOG_PREVIEW_PATH=/tmp/log.html cargo test renders_full_log_page
TICKETS_PREVIEW_PATH=/tmp/tickets.html cargo test renders_tickets_page
STATS_PREVIEW_PATH=/tmp/stats.html cargo test renders_stats_page
TRANSACTIONS_PREVIEW_PATH=/tmp/transactions.html cargo test renders_transactions_page
COTISATIONS_PREVIEW_PATH=/tmp/cotisations.html cargo test renders_cotisations_page
COFFRES_PREVIEW_PATH=/tmp/coffres.html cargo test renders_coffres_page
```

**Markdown Discord** : `src/web/markdown.rs`, testé par `cargo test markdown`.
