-- CreateTable
CREATE TABLE "Transaction" (
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

-- CreateTable
CREATE TABLE "TransactionItem" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "transactionId" TEXT NOT NULL,
    "nom" TEXT NOT NULL,
    "quantite" INTEGER NOT NULL DEFAULT 1,
    "prixUnitaire" REAL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "TransactionItem_transactionId_fkey" FOREIGN KEY ("transactionId") REFERENCES "Transaction" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "PresenceConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "defaultRoleId" TEXT,
    "exceptionRoleId" TEXT,
    "reminderChannelId" TEXT,
    "updatedAt" DATETIME NOT NULL
);

-- CreateTable
CREATE TABLE "Presence" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "channelId" TEXT NOT NULL,
    "messageId" TEXT,
    "titre" TEXT NOT NULL,
    "description" TEXT,
    "eventDate" DATETIME NOT NULL,
    "roleId" TEXT NOT NULL,
    "exceptionRoleId" TEXT,
    "createdById" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "closedAt" DATETIME,
    "lastReminderAt" DATETIME
);

-- CreateTable
CREATE TABLE "PresenceResponse" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "presenceId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "status" TEXT NOT NULL,
    "retardMinutes" INTEGER,
    "respondedAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" DATETIME NOT NULL,
    CONSTRAINT "PresenceResponse_presenceId_fkey" FOREIGN KEY ("presenceId") REFERENCES "Presence" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "PresenceResponseChange" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "responseId" TEXT NOT NULL,
    "fromStatus" TEXT,
    "toStatus" TEXT NOT NULL,
    "changedAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "PresenceResponseChange_responseId_fkey" FOREIGN KEY ("responseId") REFERENCES "PresenceResponse" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateIndex
CREATE INDEX "Transaction_guildId_channelId_idx" ON "Transaction"("guildId", "channelId");

-- CreateIndex
CREATE INDEX "Transaction_messageId_idx" ON "Transaction"("messageId");

-- CreateIndex
CREATE INDEX "TransactionItem_transactionId_idx" ON "TransactionItem"("transactionId");

-- CreateIndex
CREATE INDEX "Presence_guildId_channelId_idx" ON "Presence"("guildId", "channelId");

-- CreateIndex
CREATE INDEX "Presence_messageId_idx" ON "Presence"("messageId");

-- CreateIndex
CREATE INDEX "Presence_eventDate_idx" ON "Presence"("eventDate");

-- CreateIndex
CREATE INDEX "PresenceResponse_presenceId_status_idx" ON "PresenceResponse"("presenceId", "status");

-- CreateIndex
CREATE UNIQUE INDEX "PresenceResponse_presenceId_userId_key" ON "PresenceResponse"("presenceId", "userId");

-- CreateIndex
CREATE INDEX "PresenceResponseChange_responseId_idx" ON "PresenceResponseChange"("responseId");
