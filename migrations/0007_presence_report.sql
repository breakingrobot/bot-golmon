-- Rapport hebdomadaire d'assiduité (lundi 09:00, heure de Paris) : salon,
-- seuil d'alerte en % (50 par défaut) et date du dernier envoi, pour ne
-- l'envoyer qu'une fois par semaine même après un redémarrage.
ALTER TABLE "PresenceConfig" ADD COLUMN "reportChannelId" TEXT;
ALTER TABLE "PresenceConfig" ADD COLUMN "alertThreshold" INTEGER;
ALTER TABLE "PresenceConfig" ADD COLUMN "lastReportAt" DATETIME;
