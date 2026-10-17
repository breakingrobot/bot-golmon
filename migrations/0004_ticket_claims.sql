-- Prise en charge des tickets (module modmail). Stockée dans la base principale :
-- la base des tickets garde le schéma exact de modmailbot.
CREATE TABLE IF NOT EXISTS "TicketClaim" (
    "threadId" TEXT NOT NULL PRIMARY KEY,
    "staffId" TEXT NOT NULL,
    "staffName" TEXT NOT NULL,
    "claimedAt" DATETIME NOT NULL
);
CREATE INDEX IF NOT EXISTS "TicketClaim_staffId_idx" ON "TicketClaim"("staffId");
