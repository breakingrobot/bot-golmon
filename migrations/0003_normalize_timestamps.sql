-- Les dates sont stockées en texte ISO 8601 UTC (`2026-08-10T21:00:00.000Z`),
-- le format qu'utilisait l'adaptateur Prisma par défaut. Selon sa configuration,
-- Prisma a aussi pu écrire des millisecondes epoch (entiers) : on les convertit
-- pour que toutes les comparaisons de dates restent de simples comparaisons
-- de chaînes, indexables.

UPDATE "Transaction" SET "createdAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "createdAt" / 1000.0, 'unixepoch') WHERE typeof("createdAt") = 'integer';
UPDATE "Transaction" SET "updatedAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "updatedAt" / 1000.0, 'unixepoch') WHERE typeof("updatedAt") = 'integer';
UPDATE "TransactionItem" SET "createdAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "createdAt" / 1000.0, 'unixepoch') WHERE typeof("createdAt") = 'integer';
UPDATE "Presence" SET "eventDate" = strftime('%Y-%m-%dT%H:%M:%fZ', "eventDate" / 1000.0, 'unixepoch') WHERE typeof("eventDate") = 'integer';
UPDATE "Presence" SET "createdAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "createdAt" / 1000.0, 'unixepoch') WHERE typeof("createdAt") = 'integer';
UPDATE "Presence" SET "closedAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "closedAt" / 1000.0, 'unixepoch') WHERE typeof("closedAt") = 'integer';
UPDATE "Presence" SET "lastReminderAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "lastReminderAt" / 1000.0, 'unixepoch') WHERE typeof("lastReminderAt") = 'integer';
UPDATE "PresenceResponse" SET "respondedAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "respondedAt" / 1000.0, 'unixepoch') WHERE typeof("respondedAt") = 'integer';
UPDATE "PresenceResponseChange" SET "changedAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "changedAt" / 1000.0, 'unixepoch') WHERE typeof("changedAt") = 'integer';
UPDATE "Absence" SET "startsAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "startsAt" / 1000.0, 'unixepoch') WHERE typeof("startsAt") = 'integer';
UPDATE "Absence" SET "endsAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "endsAt" / 1000.0, 'unixepoch') WHERE typeof("endsAt") = 'integer';
UPDATE "Absence" SET "endedAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "endedAt" / 1000.0, 'unixepoch') WHERE typeof("endedAt") = 'integer';
UPDATE "Absence" SET "createdAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "createdAt" / 1000.0, 'unixepoch') WHERE typeof("createdAt") = 'integer';
UPDATE "PresenceBlocklist" SET "createdAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "createdAt" / 1000.0, 'unixepoch') WHERE typeof("createdAt") = 'integer';
UPDATE "Vehicle" SET "createdAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "createdAt" / 1000.0, 'unixepoch') WHERE typeof("createdAt") = 'integer';
UPDATE "RecrueSession" SET "weekStart" = strftime('%Y-%m-%dT%H:%M:%fZ', "weekStart" / 1000.0, 'unixepoch') WHERE typeof("weekStart") = 'integer';
UPDATE "RecrueSession" SET "closedAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "closedAt" / 1000.0, 'unixepoch') WHERE typeof("closedAt") = 'integer';
UPDATE "RecrueSession" SET "createdAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "createdAt" / 1000.0, 'unixepoch') WHERE typeof("createdAt") = 'integer';
UPDATE "RecrueEntry" SET "vetoAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "vetoAt" / 1000.0, 'unixepoch') WHERE typeof("vetoAt") = 'integer';
UPDATE "RecrueEntry" SET "createdAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "createdAt" / 1000.0, 'unixepoch') WHERE typeof("createdAt") = 'integer';
UPDATE "RecrueVote" SET "createdAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "createdAt" / 1000.0, 'unixepoch') WHERE typeof("createdAt") = 'integer';
UPDATE "RecrueChannel" SET "createdAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "createdAt" / 1000.0, 'unixepoch') WHERE typeof("createdAt") = 'integer';
UPDATE "PhoneNumber" SET "createdAt" = strftime('%Y-%m-%dT%H:%M:%fZ', "createdAt" / 1000.0, 'unixepoch') WHERE typeof("createdAt") = 'integer';
