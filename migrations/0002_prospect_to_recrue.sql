-- Renommage du module Prospect en Recrue. Écrit à la main en RENAME pour
-- conserver toutes les données (sessions, avis, vétos, salons d'essai).
-- SQLite met à jour les clés étrangères qui pointent vers les tables renommées.

ALTER TABLE "ProspectConfig" RENAME TO "RecrueConfig";
ALTER TABLE "RecrueConfig" RENAME COLUMN "prospectRoleId" TO "recrueRoleId";

ALTER TABLE "ProspectSession" RENAME TO "RecrueSession";
DROP INDEX IF EXISTS "ProspectSession_guildId_closedAt_idx";
DROP INDEX IF EXISTS "ProspectSession_guildId_weekStart_key";
CREATE INDEX "RecrueSession_guildId_closedAt_idx" ON "RecrueSession"("guildId", "closedAt");
CREATE UNIQUE INDEX "RecrueSession_guildId_weekStart_key" ON "RecrueSession"("guildId", "weekStart");

ALTER TABLE "ProspectEntry" RENAME TO "RecrueEntry";
DROP INDEX IF EXISTS "ProspectEntry_guildId_userId_idx";
DROP INDEX IF EXISTS "ProspectEntry_sessionId_userId_key";
CREATE INDEX "RecrueEntry_guildId_userId_idx" ON "RecrueEntry"("guildId", "userId");
CREATE UNIQUE INDEX "RecrueEntry_sessionId_userId_key" ON "RecrueEntry"("sessionId", "userId");

ALTER TABLE "ProspectVote" RENAME TO "RecrueVote";
DROP INDEX IF EXISTS "ProspectVote_entryId_choix_idx";
DROP INDEX IF EXISTS "ProspectVote_entryId_voterId_key";
CREATE INDEX "RecrueVote_entryId_choix_idx" ON "RecrueVote"("entryId", "choix");
CREATE UNIQUE INDEX "RecrueVote_entryId_voterId_key" ON "RecrueVote"("entryId", "voterId");

ALTER TABLE "ProspectChannel" RENAME TO "RecrueChannel";
DROP INDEX IF EXISTS "ProspectChannel_guildId_userId_key";
CREATE UNIQUE INDEX "RecrueChannel_guildId_userId_key" ON "RecrueChannel"("guildId", "userId");
