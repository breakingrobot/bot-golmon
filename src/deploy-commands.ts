import { readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import "dotenv/config";
import { REST, Routes } from "discord.js";
import type { Command } from "./types/command.js";
import { logger } from "./lib/logger.js";
import { getWhitelistedGuildIds } from "./lib/guildWhitelist.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function findCommandFiles(dir: string): string[] {
  const entries = readdirSync(dir, { withFileTypes: true });
  const files: string[] = [];

  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...findCommandFiles(fullPath));
    } else if (entry.isFile() && (entry.name.endsWith(".js") || entry.name.endsWith(".ts"))) {
      files.push(fullPath);
    }
  }

  return files;
}

const token = process.env.DISCORD_TOKEN;
const clientId = process.env.DISCORD_CLIENT_ID;
const guildIds = getWhitelistedGuildIds();

if (!token || !clientId) {
  throw new Error("DISCORD_TOKEN et DISCORD_CLIENT_ID sont requis");
}

const commandsDir = path.join(__dirname, "commands");
const files = findCommandFiles(commandsDir);
const body = [];

for (const file of files) {
  const imported = (await import(pathToFileURL(file).href)) as { command?: Command };
  if (!imported.command) {
    continue;
  }
  body.push(imported.command.data.toJSON());
}

const rest = new REST().setToken(token);

if (guildIds.length > 0) {
  for (const guildId of guildIds) {
    await rest.put(Routes.applicationGuildCommands(clientId, guildId), { body });
    logger.info(`${body.length} commande(s) déployée(s) sur la guilde ${guildId}`);
  }
} else {
  await rest.put(Routes.applicationCommands(clientId), { body });
  logger.info(`${body.length} commande(s) déployée(s) globalement (aucune whitelist configurée)`);
}
