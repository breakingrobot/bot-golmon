/*
  Warnings:

  - You are about to drop the column `dashboardMessageId` on the `ProspectConfig` table. All the data in the column will be lost.

*/
-- RedefineTables
PRAGMA defer_foreign_keys=ON;
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_ProspectConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "prospectRoleId" TEXT,
    "gradeRoleId" TEXT,
    "leadRoleId" TEXT,
    "categoryId" TEXT,
    "dashboardChannelId" TEXT,
    "updatedAt" DATETIME NOT NULL
);
INSERT INTO "new_ProspectConfig" ("categoryId", "dashboardChannelId", "gradeRoleId", "guildId", "leadRoleId", "prospectRoleId", "updatedAt") SELECT "categoryId", "dashboardChannelId", "gradeRoleId", "guildId", "leadRoleId", "prospectRoleId", "updatedAt" FROM "ProspectConfig";
DROP TABLE "ProspectConfig";
ALTER TABLE "new_ProspectConfig" RENAME TO "ProspectConfig";
PRAGMA foreign_keys=ON;
PRAGMA defer_foreign_keys=OFF;
