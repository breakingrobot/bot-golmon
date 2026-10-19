-- Stock agrégé par propriété et par objet, tenu à jour à chaque mouvement :
-- les pages et l'autocomplétion ne relisent plus tout l'historique.
CREATE TABLE IF NOT EXISTS "CoffreStock" (
    "guildId" TEXT NOT NULL,
    "propertyId" TEXT NOT NULL,
    "item" TEXT NOT NULL,
    "deposited" INTEGER NOT NULL DEFAULT 0,
    "withdrawn" INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY ("guildId", "propertyId", "item")
);
INSERT OR REPLACE INTO "CoffreStock" ("guildId", "propertyId", "item", "deposited", "withdrawn")
SELECT "guildId", "propertyId", "item",
       COALESCE(SUM(CASE WHEN "kind" = 'DEPOSIT' THEN "amount" END), 0),
       COALESCE(SUM(CASE WHEN "kind" = 'WITHDRAW' THEN "amount" END), 0)
FROM "CoffreMovement" GROUP BY "guildId", "propertyId", "item";

-- Index alignés sur les filtres de l'historique, triés par date.
DROP INDEX IF EXISTS "CoffreMovement_guildId_property_item_idx";
DROP INDEX IF EXISTS "CoffreMovement_guildId_player_idx";
CREATE INDEX IF NOT EXISTS "CoffreMovement_guildId_property_at_idx" ON "CoffreMovement"("guildId", "propertyId", "at");
CREATE INDEX IF NOT EXISTS "CoffreMovement_guildId_player_at_idx" ON "CoffreMovement"("guildId", "playerGameId", "at");
CREATE INDEX IF NOT EXISTS "CoffreMovement_guildId_item_at_idx" ON "CoffreMovement"("guildId", "item", "at");

-- Webhook du jeu retenu pour chaque salon : les messages des autres webhooks
-- ou applications (encadrés imités) sont ignorés.
ALTER TABLE "CoffreChannel" ADD COLUMN "webhookId" TEXT;
