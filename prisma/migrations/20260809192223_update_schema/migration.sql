-- RedefineTables
PRAGMA defer_foreign_keys=ON;
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_Vehicle" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "plate" TEXT NOT NULL,
    "ownerId" TEXT NOT NULL,
    "ownerIsRole" BOOLEAN NOT NULL DEFAULT false,
    "modele" TEXT,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" DATETIME NOT NULL
);
INSERT INTO "new_Vehicle" ("createdAt", "guildId", "id", "modele", "ownerId", "plate", "updatedAt") SELECT "createdAt", "guildId", "id", "modele", "ownerId", "plate", "updatedAt" FROM "Vehicle";
DROP TABLE "Vehicle";
ALTER TABLE "new_Vehicle" RENAME TO "Vehicle";
CREATE INDEX "Vehicle_guildId_ownerId_idx" ON "Vehicle"("guildId", "ownerId");
CREATE UNIQUE INDEX "Vehicle_guildId_plate_key" ON "Vehicle"("guildId", "plate");
PRAGMA foreign_keys=ON;
PRAGMA defer_foreign_keys=OFF;
