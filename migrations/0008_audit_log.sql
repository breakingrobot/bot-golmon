-- Journal des actions du staff (votes forcés, suppressions, configuration…).
CREATE TABLE IF NOT EXISTS "AuditLog" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "guildId" TEXT NOT NULL,
    "actorId" TEXT NOT NULL,
    "actorName" TEXT NOT NULL,
    "action" TEXT NOT NULL,
    "targetId" TEXT,
    "details" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL
);
CREATE INDEX IF NOT EXISTS "AuditLog_guildId_createdAt_idx" ON "AuditLog"("guildId", "createdAt");

-- Réglages communs à tout le serveur.
CREATE TABLE IF NOT EXISTS "GuildSettings" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    -- Salon où publier le journal du staff (NULL : journal seulement sur le site).
    "auditChannelId" TEXT,
    "updatedAt" DATETIME NOT NULL
);
