-- Renommage exceptionRoleId -> absenceRoleId.
-- Ecrit a la main : Prisma aurait supprime puis recree la colonne, perdant la
-- configuration existante et le role des presences deja creees.
ALTER TABLE "PresenceConfig" RENAME COLUMN "exceptionRoleId" TO "absenceRoleId";
ALTER TABLE "Presence" RENAME COLUMN "exceptionRoleId" TO "absenceRoleId";

-- AlterTable
ALTER TABLE "PresenceConfig" ADD COLUMN "absenceChannelId" TEXT;

-- CreateTable
CREATE TABLE "Absence" (
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

-- CreateTable
CREATE TABLE "PresenceBlocklist" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "raison" TEXT,
    "addedById" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateIndex
CREATE INDEX "Absence_guildId_userId_idx" ON "Absence"("guildId", "userId");

-- CreateIndex
CREATE INDEX "Absence_endedAt_idx" ON "Absence"("endedAt");

-- CreateIndex
CREATE UNIQUE INDEX "PresenceBlocklist_guildId_userId_key" ON "PresenceBlocklist"("guildId", "userId");
