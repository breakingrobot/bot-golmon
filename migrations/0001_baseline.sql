-- Schéma de référence, identique à la dernière migration Prisma de la version
-- TypeScript. Tout est en IF NOT EXISTS : sur une base existante (ancienne
-- version du bot), cette migration ne touche à rien ; sur une base neuve,
-- elle crée le schéma complet.

CREATE TABLE IF NOT EXISTS "Transaction" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "channelId" TEXT NOT NULL,
    "messageId" TEXT,
    "groupe" TEXT NOT NULL,
    "type" TEXT NOT NULL,
    "prix" REAL NOT NULL,
    "statut" TEXT NOT NULL DEFAULT 'PREPARATION',
    "createdById" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" DATETIME NOT NULL
);
CREATE INDEX IF NOT EXISTS "Transaction_guildId_channelId_idx" ON "Transaction"("guildId", "channelId");
CREATE INDEX IF NOT EXISTS "Transaction_guildId_statut_idx" ON "Transaction"("guildId", "statut");
CREATE INDEX IF NOT EXISTS "Transaction_messageId_idx" ON "Transaction"("messageId");

CREATE TABLE IF NOT EXISTS "TransactionItem" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "transactionId" TEXT NOT NULL,
    "nom" TEXT NOT NULL,
    "quantite" INTEGER NOT NULL DEFAULT 1,
    "prixUnitaire" REAL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "TransactionItem_transactionId_fkey" FOREIGN KEY ("transactionId") REFERENCES "Transaction" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE INDEX IF NOT EXISTS "TransactionItem_transactionId_idx" ON "TransactionItem"("transactionId");

CREATE TABLE IF NOT EXISTS "TransactionConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "dashboardChannelId" TEXT,
    "dashboardMessageId" TEXT,
    "updatedAt" DATETIME NOT NULL,
    "pingRoleId" TEXT
);

CREATE TABLE IF NOT EXISTS "PresenceConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "defaultRoleId" TEXT,
    "absenceRoleId" TEXT,
    "reminderChannelId" TEXT,
    "updatedAt" DATETIME NOT NULL,
    "presenceChannelId" TEXT,
    "absenceChannelId" TEXT
);

CREATE TABLE IF NOT EXISTS "Presence" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "channelId" TEXT NOT NULL,
    "messageId" TEXT,
    "titre" TEXT NOT NULL,
    "description" TEXT,
    "eventDate" DATETIME NOT NULL,
    "roleId" TEXT NOT NULL,
    "absenceRoleId" TEXT,
    "createdById" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "closedAt" DATETIME,
    "lastReminderAt" DATETIME
);
CREATE INDEX IF NOT EXISTS "Presence_guildId_channelId_idx" ON "Presence"("guildId", "channelId");
CREATE INDEX IF NOT EXISTS "Presence_messageId_idx" ON "Presence"("messageId");
CREATE INDEX IF NOT EXISTS "Presence_eventDate_idx" ON "Presence"("eventDate");

CREATE TABLE IF NOT EXISTS "PresenceResponse" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "presenceId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "status" TEXT NOT NULL,
    "retardMinutes" INTEGER,
    "respondedAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" DATETIME NOT NULL,
    CONSTRAINT "PresenceResponse_presenceId_fkey" FOREIGN KEY ("presenceId") REFERENCES "Presence" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE INDEX IF NOT EXISTS "PresenceResponse_presenceId_status_idx" ON "PresenceResponse"("presenceId", "status");
CREATE UNIQUE INDEX IF NOT EXISTS "PresenceResponse_presenceId_userId_key" ON "PresenceResponse"("presenceId", "userId");
CREATE INDEX IF NOT EXISTS "PresenceResponse_userId_idx" ON "PresenceResponse"("userId");

CREATE TABLE IF NOT EXISTS "PresenceResponseChange" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "responseId" TEXT NOT NULL,
    "fromStatus" TEXT,
    "toStatus" TEXT NOT NULL,
    "changedAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "PresenceResponseChange_responseId_fkey" FOREIGN KEY ("responseId") REFERENCES "PresenceResponse" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE INDEX IF NOT EXISTS "PresenceResponseChange_responseId_idx" ON "PresenceResponseChange"("responseId");

CREATE TABLE IF NOT EXISTS "Absence" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "motif" TEXT NOT NULL,
    "startsAt" DATETIME NOT NULL,
    "endsAt" DATETIME NOT NULL,
    "createdById" TEXT NOT NULL,
    "endedAt" DATETIME,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS "Absence_guildId_userId_idx" ON "Absence"("guildId", "userId");
CREATE INDEX IF NOT EXISTS "Absence_endedAt_idx" ON "Absence"("endedAt");

CREATE TABLE IF NOT EXISTS "PresenceBlocklist" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "raison" TEXT,
    "addedById" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX IF NOT EXISTS "PresenceBlocklist_guildId_userId_key" ON "PresenceBlocklist"("guildId", "userId");

CREATE TABLE IF NOT EXISTS "Vehicle" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "plate" TEXT NOT NULL,
    "ownerId" TEXT NOT NULL,
    "ownerIsRole" BOOLEAN NOT NULL DEFAULT false,
    "modele" TEXT,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" DATETIME NOT NULL
);
CREATE INDEX IF NOT EXISTS "Vehicle_guildId_ownerId_idx" ON "Vehicle"("guildId", "ownerId");
CREATE UNIQUE INDEX IF NOT EXISTS "Vehicle_guildId_plate_key" ON "Vehicle"("guildId", "plate");

CREATE TABLE IF NOT EXISTS "VehicleConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "dashboardChannelId" TEXT,
    "dashboardMessageId" TEXT,
    "updatedAt" DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS "ProspectConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "prospectRoleId" TEXT,
    "gradeRoleId" TEXT,
    "leadRoleId" TEXT,
    "categoryId" TEXT,
    "dashboardChannelId" TEXT,
    "updatedAt" DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS "ProspectSession" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "weekStart" DATETIME NOT NULL,
    "closedAt" DATETIME,
    "messageId" TEXT,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS "ProspectSession_guildId_closedAt_idx" ON "ProspectSession"("guildId", "closedAt");
CREATE UNIQUE INDEX IF NOT EXISTS "ProspectSession_guildId_weekStart_key" ON "ProspectSession"("guildId", "weekStart");

CREATE TABLE IF NOT EXISTS "ProspectEntry" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "sessionId" TEXT NOT NULL,
    "guildId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "vetoById" TEXT,
    "vetoMotif" TEXT,
    "vetoAt" DATETIME,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "ProspectEntry_sessionId_fkey" FOREIGN KEY ("sessionId") REFERENCES "ProspectSession" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE INDEX IF NOT EXISTS "ProspectEntry_guildId_userId_idx" ON "ProspectEntry"("guildId", "userId");
CREATE UNIQUE INDEX IF NOT EXISTS "ProspectEntry_sessionId_userId_key" ON "ProspectEntry"("sessionId", "userId");

CREATE TABLE IF NOT EXISTS "ProspectVote" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "entryId" TEXT NOT NULL,
    "voterId" TEXT NOT NULL,
    "choix" TEXT NOT NULL,
    "motif" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" DATETIME NOT NULL,
    CONSTRAINT "ProspectVote_entryId_fkey" FOREIGN KEY ("entryId") REFERENCES "ProspectEntry" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE INDEX IF NOT EXISTS "ProspectVote_entryId_choix_idx" ON "ProspectVote"("entryId", "choix");
CREATE UNIQUE INDEX IF NOT EXISTS "ProspectVote_entryId_voterId_key" ON "ProspectVote"("entryId", "voterId");

CREATE TABLE IF NOT EXISTS "ProspectChannel" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "channelId" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX IF NOT EXISTS "ProspectChannel_guildId_userId_key" ON "ProspectChannel"("guildId", "userId");

CREATE TABLE IF NOT EXISTS "PhoneNumber" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "numero" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" DATETIME NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS "PhoneNumber_guildId_userId_key" ON "PhoneNumber"("guildId", "userId");
CREATE UNIQUE INDEX IF NOT EXISTS "PhoneNumber_guildId_numero_key" ON "PhoneNumber"("guildId", "numero");
CREATE INDEX IF NOT EXISTS "PhoneNumber_guildId_numero_idx" ON "PhoneNumber"("guildId", "numero");

CREATE TABLE IF NOT EXISTS "PhoneConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "dashboardChannelId" TEXT,
    "dashboardMessageId" TEXT,
    "updatedAt" DATETIME NOT NULL
);
