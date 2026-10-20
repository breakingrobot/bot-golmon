-- Rappels personnels en message privé, désactivés par défaut (`/config rappels`).
CREATE TABLE IF NOT EXISTS "RappelConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "enabled" INTEGER NOT NULL DEFAULT 0,
    -- MP quand une présence sans réponse commence dans moins de N heures.
    "presenceHours" INTEGER NOT NULL DEFAULT 2,
    -- MP le samedi soir aux membres sous l'objectif de cotisation.
    "objective" INTEGER NOT NULL DEFAULT 1,
    "updatedAt" DATETIME NOT NULL
);

-- Choix de chaque membre (`/rappels`) et messages privés refusés par Discord.
CREATE TABLE IF NOT EXISTS "RappelMember" (
    "guildId" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    -- 1 : rappels coupés par le membre.
    "presenceOff" INTEGER NOT NULL DEFAULT 0,
    "objectiveOff" INTEGER NOT NULL DEFAULT 0,
    -- Dernier MP refusé (messages privés fermés) : pas de nouvel essai pendant quelques jours.
    "dmClosedAt" DATETIME,
    "updatedAt" DATETIME NOT NULL,
    PRIMARY KEY ("guildId", "userId")
);

-- Rappels déjà envoyés (ou tentés) : un seul par présence, ou par semaine
-- d'objectif, et par membre, même après un redémarrage du bot.
CREATE TABLE IF NOT EXISTS "RappelSent" (
    "guildId" TEXT NOT NULL,
    -- presence | objective
    "kind" TEXT NOT NULL,
    -- Identifiant de la présence, ou début de la semaine de l'objectif.
    "ref" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "sentAt" DATETIME NOT NULL,
    PRIMARY KEY ("kind", "ref", "userId")
);
CREATE INDEX IF NOT EXISTS "RappelSent_sentAt_idx" ON "RappelSent"("sentAt");
