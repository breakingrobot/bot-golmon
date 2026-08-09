-- AlterTable
ALTER TABLE "PresenceConfig" ADD COLUMN "presenceChannelId" TEXT;

-- CreateTable
CREATE TABLE "TransactionConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "dashboardChannelId" TEXT,
    "dashboardMessageId" TEXT,
    "updatedAt" DATETIME NOT NULL
);

-- CreateIndex
CREATE INDEX "PresenceResponse_userId_idx" ON "PresenceResponse"("userId");

-- CreateIndex
CREATE INDEX "Transaction_guildId_statut_idx" ON "Transaction"("guildId", "statut");
