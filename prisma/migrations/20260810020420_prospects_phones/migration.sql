/*
  Warnings:

  - You are about to drop the column `channelId` on the `ProspectSession` table. All the data in the column will be lost.

*/
-- AlterTable
ALTER TABLE "ProspectConfig" ADD COLUMN "dashboardMessageId" TEXT;

-- RedefineTables
PRAGMA defer_foreign_keys=ON;
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_ProspectSession" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "weekStart" DATETIME NOT NULL,
    "closedAt" DATETIME,
    "messageId" TEXT,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_ProspectSession" ("closedAt", "createdAt", "guildId", "id", "messageId", "weekStart") SELECT "closedAt", "createdAt", "guildId", "id", "messageId", "weekStart" FROM "ProspectSession";
DROP TABLE "ProspectSession";
ALTER TABLE "new_ProspectSession" RENAME TO "ProspectSession";
CREATE INDEX "ProspectSession_guildId_closedAt_idx" ON "ProspectSession"("guildId", "closedAt");
CREATE UNIQUE INDEX "ProspectSession_guildId_weekStart_key" ON "ProspectSession"("guildId", "weekStart");
PRAGMA foreign_keys=ON;
PRAGMA defer_foreign_keys=OFF;

-- CreateIndex
CREATE INDEX "PhoneNumber_guildId_numero_idx" ON "PhoneNumber"("guildId", "numero");
