-- CreateTable
CREATE TABLE "Vehicle" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "guildId" TEXT NOT NULL,
    "plate" TEXT NOT NULL,
    "ownerId" TEXT NOT NULL,
    "modele" TEXT,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" DATETIME NOT NULL
);

-- CreateTable
CREATE TABLE "VehicleConfig" (
    "guildId" TEXT NOT NULL PRIMARY KEY,
    "dashboardChannelId" TEXT,
    "dashboardMessageId" TEXT,
    "updatedAt" DATETIME NOT NULL
);

-- CreateIndex
CREATE INDEX "Vehicle_guildId_ownerId_idx" ON "Vehicle"("guildId", "ownerId");

-- CreateIndex
CREATE UNIQUE INDEX "Vehicle_guildId_plate_key" ON "Vehicle"("guildId", "plate");
