import { readdirSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import type { Client } from "discord.js";
import type { Command } from "../types/command.js";
import { logger } from "./logger.js";

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

export async function loadCommands(client: Client, commandsDir: string): Promise<void> {
  const files = findCommandFiles(commandsDir);

  for (const file of files) {
    const imported = (await import(pathToFileURL(file).href)) as { command?: Command };
    const command = imported.command;

    if (!command) {
      logger.warn(`Fichier de commande sans export "command": ${file}`);
      continue;
    }

    client.commands.set(command.data.name, command);
  }

  logger.info(`${client.commands.size} commande(s) chargée(s)`);
}
