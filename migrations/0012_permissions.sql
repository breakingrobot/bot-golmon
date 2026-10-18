-- Permissions par action : rôle « membre du groupe » et niveau choisi pour
-- chaque action (sinon le niveau par défaut défini dans le code).
CREATE TABLE IF NOT EXISTS "PermissionConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "memberRoleId" TEXT,
    "updatedAt" DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS "PermissionLevel" (
    "guildId" TEXT NOT NULL,
    -- Clé de l'action, ex. « contact.ajouter ».
    "action" TEXT NOT NULL,
    -- tous | membre | grade | lead | admin
    "level" TEXT NOT NULL,
    "updatedAt" DATETIME NOT NULL,
    PRIMARY KEY ("guildId", "action")
);
