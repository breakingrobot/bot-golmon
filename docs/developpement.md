# Développement

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test

cd ui && npm ci
npm run format:check && npm run lint && npm run typecheck && npm run check:api && npm run build
```

La CI (`.github/workflows/ci.yml`) vérifie tout cela (Rust et interface) et le build Docker.

| Dossier | Contenu |
|---|---|
| `src/discord/` | Gateway, interactions, réponses, composants, cache des membres |
| `src/modules/` | Un dossier par module à slash commands ; `permissions/`, `status.rs`, `fiche.rs` |
| `src/modmail/` | Tickets : configuration, base, commandes texte, boutons, données des logs |
| `src/web/` | Serveur web : connexion Discord, service de l'interface (`spa.rs`), rendu du markdown Discord |
| `src/web/api/` | API JSON du site (une route par fichier de domaine) et spécification OpenAPI (`openapi.rs`) |
| `src/util/` | Dates (Europe/Paris), mise en forme, identifiants |
| `migrations/` | Migrations SQL de la base principale, appliquées au démarrage |
| `ui/` | Interface React : Vite, TanStack Router et Query, Tailwind CSS v4, shadcn/ui, TanStack Charts, Jotai ([README](../ui/README.md)) |

**Nouvelle colonne en base** : ajouter `migrations/000N_description.sql`, ne jamais modifier une migration
déjà déployée.

**Interface en local** : `cd ui && npm run dev:mock` (sans le bot, données fictives) ou `npm run dev` (API relayée
vers le bot sur `http://localhost:8890`). Le bot sert `ui/dist` (variable `WEB_UI_DIR`) : `npm run build` avant de le
lancer pour voir le site compilé.

**Changer l'API** : modifier `src/web/api/`, puis régénérer la spécification et vérifier l'interface :

```bash
UPDATE_OPENAPI=1 cargo test openapi_spec_is_up_to_date   # réécrit ui/openapi.json
cd ui && npm run check:api                               # client Hey API ↔ types de l'interface
```

**Essai dans un navigateur** du vrai serveur web, sans Discord (connexion désactivée, 120 s) :

```bash
cd ui && npm run build && cd ..
UI_SMOKE_PORT=8899 cargo test serve_for_browser -- --ignored --nocapture
```

**Markdown Discord** : `src/web/markdown.rs`, testé par `cargo test markdown`.
