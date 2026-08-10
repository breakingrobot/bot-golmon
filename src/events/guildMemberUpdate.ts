import type { BotEvent } from "../types/event.js";
import { logger } from "../lib/logger.js";
import { ensureProspectChannel } from "../modules/prospects/channels.js";
import {
  addEntry,
  ensureCurrentSession,
  getProspectConfig,
  removeEntry,
  syncProspectDashboard,
} from "../modules/prospects/service.js";

/**
 * Ouvre (ou ferme) le suivi d'un prospect quand le rôle configuré lui est
 * attribué ou retiré. Le salon privé, lui, est conservé quand le rôle est
 * retiré : l'historique de la période d'essai reste consultable.
 *
 * `oldMember` peut être partiel si le membre n'était pas en cache — son cache
 * de rôles serait alors vide et ferait passer un simple changement de pseudo
 * pour une attribution de rôle. On ignore donc ces cas.
 */
export const event: BotEvent<"guildMemberUpdate"> = {
  name: "guildMemberUpdate",
  async execute(oldMember, newMember) {
    if (oldMember.partial) {
      return;
    }

    try {
      const config = await getProspectConfig(newMember.guild.id);
      if (!config?.prospectRoleId) {
        return;
      }

      const had = oldMember.roles.cache.has(config.prospectRoleId);
      const has = newMember.roles.cache.has(config.prospectRoleId);
      if (had === has) {
        return;
      }

      const session = await ensureCurrentSession(newMember.guild);

      if (has) {
        await ensureProspectChannel(newMember.guild, newMember.id);
        await addEntry(session.id, newMember.guild.id, newMember.id);
      } else {
        await removeEntry(session.id, newMember.id);
      }

      await syncProspectDashboard(newMember.client, newMember.guild.id);
      logger.info(
        `Prospect ${newMember.user.tag} ${has ? "ajouté au" : "retiré du"} suivi de la semaine`,
      );
    } catch (error) {
      logger.error(`Suivi du prospect ${newMember.id} échoué`, error);
    }
  },
};
