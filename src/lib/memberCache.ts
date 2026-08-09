import type { Guild } from "discord.js";
import { logger } from "./logger.js";

/**
 * `guild.members.fetch()` envoie un opcode 8 (REQUEST_GUILD_MEMBERS) sur la
 * gateway, fortement rate-limité. Avec l'intent GuildMembers, discord.js
 * maintient ensuite `guild.members.cache` à jour tout seul via les events
 * GUILD_MEMBER_ADD/UPDATE/REMOVE : un seul fetch initial suffit donc.
 *
 * Ce module garantit qu'on ne refetch pas plus d'une fois par TTL et par
 * guilde, et que des appels concurrents partagent la même requête.
 */
const TTL_MS = 30 * 60 * 1000;

const lastFetchAt = new Map<string, number>();
const inFlight = new Map<string, Promise<void>>();

export async function ensureMembersCached(guild: Guild): Promise<void> {
  const last = lastFetchAt.get(guild.id) ?? 0;
  if (Date.now() - last < TTL_MS) {
    return;
  }

  const pending = inFlight.get(guild.id);
  if (pending) {
    return pending;
  }

  let request: Promise<void>;
  try {
    request = guild.members
      .fetch()
      .then(() => {
        lastFetchAt.set(guild.id, Date.now());
        logger.info(`Cache membres rafraîchi pour ${guild.name} (${guild.memberCount} membres)`);
      })
      .catch((error: unknown) => {
        // En cas d'erreur (ex: GatewayRateLimitError opcode 8), on attend 1 minute avant de réessayer
        // pour éviter de spammer l'API de Discord et se faire ban.
        lastFetchAt.set(guild.id, Date.now() - TTL_MS + 60 * 1000);
        logger.error(`Échec du fetch des membres pour ${guild.name}`, error);
      })
      .finally(() => {
        inFlight.delete(guild.id);
      });
  } catch (error) {
    // Si l'erreur est jetée de façon synchrone par discord.js
    lastFetchAt.set(guild.id, Date.now() - TTL_MS + 60 * 1000);
    logger.error(`Erreur synchrone fetch membres pour ${guild.name}`, error);
    return;
  }

  inFlight.set(guild.id, request);
  return request;
}

export async function getRoleMemberIds(guild: Guild, roleId: string): Promise<string[]> {
  await ensureMembersCached(guild);
  const role = guild.roles.cache.get(roleId);
  return role ? [...role.members.keys()] : [];
}
