-- Plusieurs webhooks du jeu par salon (une propriété = un webhook, par exemple
-- « Stock gradé » et « Stock vide poche » dans le même salon). L'ancienne
-- colonne CoffreChannel.webhookId ne retenait que le premier vu : les autres
-- étaient ignorés comme des imitations.
CREATE TABLE IF NOT EXISTS "CoffreWebhook" (
    "channelId" TEXT NOT NULL,
    "webhookId" TEXT NOT NULL,
    "guildId" TEXT NOT NULL,
    -- Nom affiché du webhook au moment où il a été reconnu.
    "name" TEXT,
    "addedAt" DATETIME NOT NULL,
    PRIMARY KEY ("channelId", "webhookId")
);
INSERT OR IGNORE INTO "CoffreWebhook" ("channelId", "webhookId", "guildId", "name", "addedAt")
SELECT "channelId", "webhookId", "guildId", NULL, CURRENT_TIMESTAMP FROM "CoffreChannel" WHERE "webhookId" IS NOT NULL;
