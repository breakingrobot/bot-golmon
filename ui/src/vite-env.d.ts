// Variables d'environnement lues par l'interface (remplacées à la compilation par Vite).

// Fusion de déclarations avec `vite/client` : seule une interface le permet.
// oxlint-disable-next-line typescript/consistent-type-definitions
interface ImportMetaEnv {
  /** `1` : maquette de l'API (`npm run dev:mock`), exclue du build de production. */
  readonly VITE_MOCK?: string
}
