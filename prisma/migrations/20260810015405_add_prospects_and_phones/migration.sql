-- CreateTable
CREATE TABLE "ProspectConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "prospectRoleId" TEXT,
    "gradeRoleId" TEXT,
    "leadRoleId" TEXT,
    "categoryId" TEXT,
    "dashboardChannelId" TEXT,
    "updatedAt" DATETIME NOT NULL
);

-- CreateTable
CREATE TABLE "ProspectSession" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "weekStart" DATETIME NOT NULL,
    "closedAt" DATETIME,
    "channelId" TEXT,
    "messageId" TEXT,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "ProspectEntry" (
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

-- CreateTable
CREATE TABLE "ProspectVote" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "entryId" TEXT NOT NULL,
    "voterId" TEXT NOT NULL,
    "choix" TEXT NOT NULL,
    "motif" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" DATETIME NOT NULL,
    CONSTRAINT "ProspectVote_entryId_fkey" FOREIGN KEY ("entryId") REFERENCES "ProspectEntry" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "ProspectChannel" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "channelId" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "PhoneNumber" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "numero" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" DATETIME NOT NULL
);

-- CreateTable
CREATE TABLE "PhoneConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "dashboardChannelId" TEXT,
    "dashboardMessageId" TEXT,
    "updatedAt" DATETIME NOT NULL
);

-- CreateIndex
CREATE INDEX "ProspectSession_guildId_closedAt_idx" ON "ProspectSession"("guildId", "closedAt");

-- CreateIndex
CREATE UNIQUE INDEX "ProspectSession_guildId_weekStart_key" ON "ProspectSession"("guildId", "weekStart");

-- CreateIndex
CREATE INDEX "ProspectEntry_guildId_userId_idx" ON "ProspectEntry"("guildId", "userId");

-- CreateIndex
CREATE UNIQUE INDEX "ProspectEntry_sessionId_userId_key" ON "ProspectEntry"("sessionId", "userId");

-- CreateIndex
CREATE INDEX "ProspectVote_entryId_choix_idx" ON "ProspectVote"("entryId", "choix");

-- CreateIndex
CREATE UNIQUE INDEX "ProspectVote_entryId_voterId_key" ON "ProspectVote"("entryId", "voterId");

-- CreateIndex
CREATE UNIQUE INDEX "ProspectChannel_guildId_userId_key" ON "ProspectChannel"("guildId", "userId");

-- CreateIndex
CREATE UNIQUE INDEX "PhoneNumber_guildId_userId_key" ON "PhoneNumber"("guildId", "userId");

-- CreateIndex
CREATE UNIQUE INDEX "PhoneNumber_guildId_numero_key" ON "PhoneNumber"("guildId", "numero");
