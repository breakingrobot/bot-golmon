-- Note libre d'une transaction, affichée dans sa fiche.
ALTER TABLE "Transaction" ADD COLUMN "note" TEXT;

-- Modèles de transaction : pré-remplissent la saisie guidée (`/transaction nouvelle`).
CREATE TABLE IF NOT EXISTS "TransactionModele" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "nom" TEXT NOT NULL,
    -- ACHAT | VENTE
    "type" TEXT NOT NULL,
    "groupe" TEXT,
    "prix" REAL,
    -- Liste d'objets au format « 2x Pistolet @ 15000, 5x Kevlar ».
    "objets" TEXT NOT NULL DEFAULT '',
    "createdById" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS "TransactionModele_guildId_nom_key" ON "TransactionModele"("guildId", "nom" COLLATE NOCASE);
