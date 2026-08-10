import type { GuildMember, APIInteractionGuildMember } from "discord.js";
import type { ProspectConfig } from "@prisma/client";

/**
 * Contrôle d'accès du module, strictement basé sur les rôles configurés.
 *
 * Volontairement, « Gérer le serveur » n'ouvre *aucun* droit ici : un
 * administrateur n'est pas un lead. Cette permission ne sert qu'à configurer le
 * module (`/prospect config`, `/prospect reset`).
 *
 * `interaction.member` est un `GuildMember` complet quand la guilde est en
 * cache, sinon la version brute de l'API où `roles` est un simple tableau
 * d'identifiants — d'où la normalisation ci-dessous.
 */
type InteractionMember = GuildMember | APIInteractionGuildMember | null;

function roleIds(member: InteractionMember): string[] {
  if (!member) {
    return [];
  }
  return Array.isArray(member.roles) ? member.roles : [...member.roles.cache.keys()];
}

function hasRole(member: InteractionMember, roleId: string | null | undefined): boolean {
  return roleId ? roleIds(member).includes(roleId) : false;
}

/** Le véto est réservé au rôle lead configuré. */
export function isLead(member: InteractionMember, config: ProspectConfig | null): boolean {
  return hasRole(member, config?.leadRoleId);
}

/** Un lead peut tout ce qu'un gradé peut faire, et dispose en plus du véto. */
export function isGrade(member: InteractionMember, config: ProspectConfig | null): boolean {
  return hasRole(member, config?.gradeRoleId) || isLead(member, config);
}

export const NOT_GRADE =
  "Seuls les membres ayant le rôle gradé (ou lead) peuvent donner un avis sur les prospects.";

export const NOT_LEAD = "Seuls les membres ayant le rôle lead peuvent poser ou lever un véto.";

export const NO_CONFIG =
  "Le module prospects n'est pas configuré. Un administrateur doit lancer `/prospect config`.";
