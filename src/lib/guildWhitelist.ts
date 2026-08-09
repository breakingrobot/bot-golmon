export function getWhitelistedGuildIds(): string[] {
  return (process.env.DISCORD_GUILD_IDS ?? "")
    .split(",")
    .map((id) => id.trim())
    .filter(Boolean);
}

/**
 * Whitelist vide = pas de restriction (pratique en dev). En prod, toujours
 * renseigner DISCORD_GUILD_IDS pour limiter le bot aux serveurs autorisés.
 */
export function isGuildAllowed(guildId: string): boolean {
  const whitelist = getWhitelistedGuildIds();
  return whitelist.length === 0 || whitelist.includes(guildId);
}
