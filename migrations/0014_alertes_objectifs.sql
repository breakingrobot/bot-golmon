-- Alertes des coffres et tableau de bord du stock.
CREATE TABLE IF NOT EXISTS "CoffreAlertConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    -- Salon des alertes (NULL : pas d'alerte).
    "channelId" TEXT,
    -- Rôle mentionné dans les alertes (seule mention autorisée).
    "roleId" TEXT,
    -- Un retrait unique d'au moins cette quantité déclenche une alerte (NULL : désactivé).
    "bigWithdrawal" INTEGER,
    -- Alerte quand un personnage relié à aucun membre retire un objet.
    "unknownPlayer" INTEGER NOT NULL DEFAULT 0,
    -- Tableau de bord permanent du stock.
    "dashboardChannelId" TEXT,
    "dashboardMessageId" TEXT,
    "updatedAt" DATETIME NOT NULL
);

-- Seuils de stock bas : alerte quand la quantité d'un objet passe sous « min ».
CREATE TABLE IF NOT EXISTS "CoffreThreshold" (
    "guildId" TEXT NOT NULL,
    -- Propriété visée ; chaîne vide = toutes les propriétés.
    "propertyId" TEXT NOT NULL DEFAULT '',
    "item" TEXT NOT NULL,
    "min" INTEGER NOT NULL,
    PRIMARY KEY ("guildId", "propertyId", "item")
);

-- Objectif hebdomadaire de cotisation et tableau de bord permanent.
ALTER TABLE "CotisationConfig" ADD COLUMN "objectiveAmount" INTEGER;
-- total | propre | sale
ALTER TABLE "CotisationConfig" ADD COLUMN "objectiveCounts" TEXT NOT NULL DEFAULT 'total';
ALTER TABLE "CotisationConfig" ADD COLUMN "objectiveReminder" INTEGER NOT NULL DEFAULT 1;
-- Dernier rappel du dimanche et dernier bilan du lundi (une fois par semaine).
ALTER TABLE "CotisationConfig" ADD COLUMN "lastReminderAt" DATETIME;
ALTER TABLE "CotisationConfig" ADD COLUMN "lastRecapAt" DATETIME;
ALTER TABLE "CotisationConfig" ADD COLUMN "dashboardChannelId" TEXT;
ALTER TABLE "CotisationConfig" ADD COLUMN "dashboardMessageId" TEXT;
-- Semaine affichée par le tableau de bord : mis à jour au changement de semaine.
ALTER TABLE "CotisationConfig" ADD COLUMN "dashboardWeek" DATETIME;
