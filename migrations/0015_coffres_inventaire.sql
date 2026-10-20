-- Inventaire manuel des coffres : un gradé saisit la quantité réelle d'un objet
-- (compté en jeu), le bot enregistre l'écart comme mouvement de correction
-- (`kind` = 'ADJUST', `amount` signé) avec son auteur. Le stock vaut
-- dépôts − retraits + corrections : un stock antérieur au suivi, ou des objets
-- jamais passés par le webhook, se rattrapent sans fausser l'historique.
ALTER TABLE "CoffreStock" ADD COLUMN "adjusted" INTEGER NOT NULL DEFAULT 0;
-- Membre Discord à l'origine d'une correction (`NULL` pour les mouvements du jeu).
ALTER TABLE "CoffreMovement" ADD COLUMN "authorId" TEXT;
