-- Annonce d'une absence (supprimée quand l'absence se termine ou est annulée).
ALTER TABLE "Absence" ADD COLUMN "channelId" TEXT;
ALTER TABLE "Absence" ADD COLUMN "messageId" TEXT;

-- Présences récurrentes : publiées automatiquement chaque semaine.
CREATE TABLE IF NOT EXISTS "PresenceSchedule" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    -- 1 = lundi … 7 = dimanche (ISO 8601).
    "weekday" INTEGER NOT NULL,
    -- Heure de l'événement, « HH:MM », heure de Paris.
    "time" TEXT NOT NULL,
    "titre" TEXT NOT NULL,
    "description" TEXT,
    -- Publication de la présence, en heures avant l'événement.
    "leadHours" INTEGER NOT NULL DEFAULT 24,
    -- Dernier événement publié : évite les doublons.
    "lastEventDate" DATETIME,
    "createdById" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL
);
CREATE INDEX IF NOT EXISTS "PresenceSchedule_guildId_idx" ON "PresenceSchedule"("guildId");
