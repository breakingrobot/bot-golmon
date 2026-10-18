-- Cotisations : ce que chaque membre rapporte au groupe, en argent propre et/ou sale.
CREATE TABLE IF NOT EXISTS "Cotisation" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "propre" INTEGER NOT NULL DEFAULT 0,
    "sale" INTEGER NOT NULL DEFAULT 0,
    "description" TEXT NOT NULL,
    -- Jour de la cotisation (minuit, heure de Paris).
    "date" DATETIME NOT NULL,
    "createdById" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL
);
CREATE INDEX IF NOT EXISTS "Cotisation_guildId_date_idx" ON "Cotisation"("guildId", "date");
CREATE INDEX IF NOT EXISTS "Cotisation_guildId_userId_idx" ON "Cotisation"("guildId", "userId");

CREATE TABLE IF NOT EXISTS "CotisationConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    -- Salon où annoncer chaque cotisation (NULL : pas d'annonce).
    "channelId" TEXT,
    "updatedAt" DATETIME NOT NULL
);
