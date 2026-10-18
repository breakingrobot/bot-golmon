-- Répertoire des contacts : autres groupes, entreprises et personnes du jeu,
-- avec leur relation au groupe et des notes réservées au staff.
CREATE TABLE IF NOT EXISTS "Contact" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    -- GROUPE | ENTREPRISE | PERSONNE
    "kind" TEXT NOT NULL,
    -- ALLIE | NEUTRE | HOSTILE
    "relation" TEXT NOT NULL,
    "phone" TEXT,
    -- Description publique (visible de tous les membres).
    "description" TEXT,
    "createdById" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL,
    "updatedAt" DATETIME NOT NULL
);
-- Un nom par serveur, sans tenir compte de la casse.
CREATE UNIQUE INDEX IF NOT EXISTS "Contact_guildId_name_key" ON "Contact"("guildId", "name" COLLATE NOCASE);

-- Notes du staff (gradés, leads) : jamais affichées aux autres membres.
CREATE TABLE IF NOT EXISTS "ContactNote" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "contactId" TEXT NOT NULL REFERENCES "Contact"("id") ON DELETE CASCADE,
    "guildId" TEXT NOT NULL,
    "authorId" TEXT NOT NULL,
    "body" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL
);
CREATE INDEX IF NOT EXISTS "ContactNote_contactId_createdAt_idx" ON "ContactNote"("contactId", "createdAt");

-- Message permanent de l'annuaire des contacts (optionnel).
CREATE TABLE IF NOT EXISTS "ContactConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "channelId" TEXT,
    "messageId" TEXT,
    "updatedAt" DATETIME NOT NULL
);
