-- Coffres : mouvements lus dans les messages du webhook du jeu (« Coffre propriété »).
CREATE TABLE IF NOT EXISTS "CoffreMovement" (
    -- Identifiant du message Discord + rang de l'encadré : un message n'est jamais compté deux fois.
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    -- Titre de l'encadré (« Coffre propriété »…), pour distinguer les sources.
    "source" TEXT NOT NULL,
    "propertyId" TEXT NOT NULL,
    "chest" INTEGER,
    "item" TEXT NOT NULL,
    "amount" INTEGER NOT NULL,
    -- DEPOSIT ou WITHDRAW.
    "kind" TEXT NOT NULL,
    "playerName" TEXT NOT NULL,
    "playerGameId" TEXT,
    "at" DATETIME NOT NULL,
    "channelId" TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS "CoffreMovement_guildId_at_idx" ON "CoffreMovement"("guildId", "at");
CREATE INDEX IF NOT EXISTS "CoffreMovement_guildId_property_item_idx" ON "CoffreMovement"("guildId", "propertyId", "item");
CREATE INDEX IF NOT EXISTS "CoffreMovement_guildId_player_idx" ON "CoffreMovement"("guildId", "playerGameId");

-- Salons où le webhook publie, et dernier message lu (rattrapage après un redémarrage).
CREATE TABLE IF NOT EXISTS "CoffreChannel" (
    "channelId" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "lastMessageId" TEXT
);

-- Noms lisibles des propriétés (« 8740 » → « Planque du port »).
CREATE TABLE IF NOT EXISTS "CoffreProperty" (
    "guildId" TEXT NOT NULL,
    "propertyId" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    PRIMARY KEY ("guildId", "propertyId")
);

-- Personnage du jeu (identifiant) → membre Discord.
CREATE TABLE IF NOT EXISTS "CoffrePlayer" (
    "guildId" TEXT NOT NULL,
    "gameId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    PRIMARY KEY ("guildId", "gameId")
);
