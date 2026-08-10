import {
  ChannelType,
  PermissionFlagsBits,
  type Guild,
  type GuildBasedChannel,
  type OverwriteResolvable,
} from "discord.js";
import { logger } from "../../lib/logger.js";
import { mention } from "../../lib/format.js";
import {
  getProspectChannelId,
  getProspectConfig,
  rememberProspectChannel,
} from "./service.js";

/** Droits accordés au prospect, aux gradés et aux leads dans le salon privé. */
const MEMBER_ALLOW = [
  PermissionFlagsBits.ViewChannel,
  PermissionFlagsBits.SendMessages,
  PermissionFlagsBits.ReadMessageHistory,
  PermissionFlagsBits.AttachFiles,
  PermissionFlagsBits.EmbedLinks,
];

export type EnsureChannelResult =
  | { ok: true; created: boolean; channelId: string }
  | { ok: false; reason: string };

/** Discord n'accepte que minuscules, chiffres et tirets dans un nom de salon. */
function slugify(name: string): string {
  const slug = name
    .toLowerCase()
    .normalize("NFD")
    // Retire les diacritiques décomposés par NFD (é -> e).
    .replaceAll(/[̀-ͯ]/g, "")
    .replaceAll(/[^a-z0-9]+/g, "-")
    .replaceAll(/^-+|-+$/g, "");
  return slug.slice(0, 90) || "prospect";
}

/**
 * Crée (ou retrouve) le salon privé d'un prospect dans la catégorie
 * configurée. Idempotent : si le salon mémorisé existe encore, il est renvoyé
 * tel quel plutôt que dupliqué.
 */
export async function ensureProspectChannel(
  guild: Guild,
  userId: string,
): Promise<EnsureChannelResult> {
  const config = await getProspectConfig(guild.id);
  if (!config?.categoryId) {
    return {
      ok: false,
      reason: "Aucune catégorie configurée pour les salons de prospects (`/prospect config`).",
    };
  }

  const knownChannelId = await getProspectChannelId(guild.id, userId);
  if (knownChannelId) {
    const existing = await guild.channels.fetch(knownChannelId).catch(() => null);
    if (existing) {
      return { ok: true, created: false, channelId: existing.id };
    }
    // Salon supprimé à la main : on en recrée un.
  }

  if (!guild.members.me?.permissions.has(PermissionFlagsBits.ManageChannels)) {
    return {
      ok: false,
      reason: "Le bot n'a pas la permission « Gérer les salons ».",
    };
  }

  const member = await guild.members.fetch(userId).catch(() => null);

  const overwrites: OverwriteResolvable[] = [
    { id: guild.roles.everyone.id, deny: [PermissionFlagsBits.ViewChannel] },
    { id: userId, allow: MEMBER_ALLOW },
    ...[config.gradeRoleId, config.leadRoleId]
      .filter((roleId): roleId is string => Boolean(roleId))
      .map((roleId) => ({ id: roleId, allow: MEMBER_ALLOW })),
  ];

  const created = await guild.channels
    .create({
      name: `essai-${slugify(member?.displayName ?? userId)}`,
      type: ChannelType.GuildText,
      parent: config.categoryId,
      permissionOverwrites: overwrites,
      reason: `Salon d'essai du prospect ${member?.user.tag ?? userId}`,
    })
    .catch((error: unknown) => {
      logger.error(`Création du salon du prospect ${userId} échouée`, error);
      return null;
    });

  if (!created) {
    return {
      ok: false,
      reason:
        "Création du salon impossible. Vérifie que la catégorie existe encore et qu'elle n'est pas pleine (50 salons maximum).",
    };
  }

  await rememberProspectChannel(guild.id, userId, created.id);
  await welcome(created, userId, config.leadRoleId, config.gradeRoleId);
  logger.info(`Salon d'essai ${created.name} créé pour ${userId}`);

  return { ok: true, created: true, channelId: created.id };
}

async function welcome(
  channel: GuildBasedChannel,
  userId: string,
  leadRoleId: string | null,
  gradeRoleId: string | null,
) {
  if (!channel.isSendable()) {
    return;
  }
  const staff = [gradeRoleId, leadRoleId].filter(Boolean).length > 0 ? "les gradés et les leads" : "l'encadrement";
  await channel
    .send({
      content: `Bienvenue ${mention(userId)} 👋\nCe salon est privé : seuls toi et ${staff} y ont accès. Pose ici tes questions pendant ta période d'essai.`,
      allowedMentions: { users: [userId] },
    })
    .catch((error: unknown) => {
      logger.warn(`Message d'accueil non envoyé dans le salon du prospect ${userId} : ${String(error)}`);
    });
}
