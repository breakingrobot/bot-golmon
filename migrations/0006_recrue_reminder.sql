-- Rappel des avis manquants (dimanche 18:00, heure de Paris) : date d'envoi,
-- pour ne l'envoyer qu'une fois par session.
ALTER TABLE "RecrueSession" ADD COLUMN "reminderSentAt" DATETIME;
