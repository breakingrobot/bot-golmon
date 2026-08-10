import {
  EmbedBuilder,
  MessageFlags,
  PermissionFlagsBits,
  type ChatInputCommandInteraction,
} from "discord.js";
import { GUILD_ONLY } from "../../../lib/replies.js";
import { mention, truncateField } from "../../../lib/format.js";
import { formatDateLabel } from "../../../lib/datetime.js";
import {
  formatEntryDetail,
  formatTally,
} from "../../../lib/embeds/prospectDashboardEmbed.js";
import { PROSPECT_COLOR, VETO_COLOR } from "../constants.js";
import { ensureProspectChannel } from "../channels.js";
import { isGrade, isLead, NOT_GRADE, NOT_LEAD } from "../permissions.js";
import {
  addEntry,
  clearVeto,
  ensureCurrentSession,
  findEntryInCurrentSession,
  getProspectConfig,
  getUserHistory,
  removeEntry,
  setVeto,
  syncProspectDashboard,
  upsertProspectConfig,
} from "../service.js";

const MANAGE_GUILD_ONLY = {
  content: "Seuls les membres avec la permission « Gérer le serveur » peuvent modifier la configuration.",
  flags: MessageFlags.Ephemeral,
} as const;

function describe(id: string | null | undefined, kind: "role" | "channel"): string {
  if (!id) {
    return "*non configuré*";
  }
  return kind === "role" ? `<@&${id}>` : `<#${id}>`;
}

/**
 * Garde commune aux sous-commandes réservées aux gradés : renvoie la guilde et
 * la config, ou `null` après avoir répondu au membre.
 */
async function requireGrade(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId || !interaction.guild) {
    await interaction.reply(GUILD_ONLY);
    return null;
  }
  const config = await getProspectConfig(interaction.guildId);
  if (!isGrade(interaction.member, config)) {
    await interaction.reply({ content: NOT_GRADE, flags: MessageFlags.Ephemeral });
    return null;
  }
  return { guild: interaction.guild, config };
}

export async function handleProspectConfig(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const prospectRole = interaction.options.getRole("role_prospect");
  const gradeRole = interaction.options.getRole("role_grade");
  const leadRole = interaction.options.getRole("role_lead");
  const category = interaction.options.getChannel("categorie");
  const dashboardChannel = interaction.options.getChannel("salon");

  // Sans aucune option, la commande sert à consulter la configuration.
  if (!prospectRole && !gradeRole && !leadRole && !category && !dashboardChannel) {
    const config = await getProspectConfig(interaction.guildId);
    const embed = new EmbedBuilder()
      .setTitle("Configuration des prospects")
      .setColor(PROSPECT_COLOR)
      .addFields(
        { name: "Rôle prospect", value: describe(config?.prospectRoleId, "role") },
        { name: "Rôle gradé (vote)", value: describe(config?.gradeRoleId, "role") },
        { name: "Rôle lead (véto)", value: describe(config?.leadRoleId, "role") },
        { name: "Catégorie des salons d'essai", value: describe(config?.categoryId, "channel") },
        { name: "Salon des avis", value: describe(config?.dashboardChannelId, "channel") },
      );

    if (!config?.prospectRoleId || !config.gradeRoleId) {
      embed.setDescription(
        "⚠️ Les rôles prospect et gradé sont indispensables au fonctionnement du module.",
      );
    }

    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
    return;
  }

  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageGuild)) {
    await interaction.reply(MANAGE_GUILD_ONLY);
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  await upsertProspectConfig({
    guildId: interaction.guildId,
    prospectRoleId: prospectRole?.id,
    gradeRoleId: gradeRole?.id,
    leadRoleId: leadRole?.id,
    categoryId: category?.id,
    dashboardChannelId: dashboardChannel?.id,
  });

  if (interaction.guild) {
    await ensureCurrentSession(interaction.guild);
    await syncProspectDashboard(interaction.client, interaction.guildId);
  }

  await interaction.editReply(
    "Configuration mise à jour. `/prospect config` sans option pour la consulter.",
  );
}

export async function handleProspectListe(interaction: ChatInputCommandInteraction) {
  const context = await requireGrade(interaction);
  if (!context) {
    return;
  }

  if (!context.config?.dashboardChannelId) {
    await interaction.reply({
      content: "Aucun salon d'avis configuré. Utilise `/prospect config salon:#salon`.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  const session = await ensureCurrentSession(context.guild);
  await syncProspectDashboard(interaction.client, context.guild.id);
  await interaction.editReply(
    `Liste des avis à jour dans <#${context.config.dashboardChannelId}> (${session.entries.length} prospect(s)).`,
  );
}

export async function handleProspectAjouter(interaction: ChatInputCommandInteraction) {
  const context = await requireGrade(interaction);
  if (!context) {
    return;
  }

  const membre = interaction.options.getUser("membre", true);
  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const session = await ensureCurrentSession(context.guild);
  await addEntry(session.id, context.guild.id, membre.id);
  await syncProspectDashboard(interaction.client, context.guild.id);

  const channel = await ensureProspectChannel(context.guild, membre.id);
  const channelNote = channel.ok
    ? ` Salon privé ${channel.created ? "créé" : "existant"} : <#${channel.channelId}>.`
    : ` ⚠️ Salon non créé — ${channel.reason}`;

  await interaction.editReply(
    `${membre.username} ajouté à la liste des prospects de la semaine.${channelNote}`,
  );
}

export async function handleProspectRetirer(interaction: ChatInputCommandInteraction) {
  const context = await requireGrade(interaction);
  if (!context) {
    return;
  }

  const membre = interaction.options.getUser("membre", true);
  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const session = await ensureCurrentSession(context.guild);
  const { count } = await removeEntry(session.id, membre.id);
  await syncProspectDashboard(interaction.client, context.guild.id);

  await interaction.editReply(
    count > 0
      ? `${membre.username} retiré de la liste de la semaine (son salon privé est conservé).`
      : `${membre.username} ne figurait pas dans la liste de la semaine.`,
  );
}

export async function handleProspectSalon(interaction: ChatInputCommandInteraction) {
  const context = await requireGrade(interaction);
  if (!context) {
    return;
  }

  const membre = interaction.options.getUser("membre", true);
  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const result = await ensureProspectChannel(context.guild, membre.id);
  if (!result.ok) {
    await interaction.editReply(`Impossible de créer le salon — ${result.reason}`);
    return;
  }
  await interaction.editReply(
    result.created
      ? `Salon <#${result.channelId}> créé pour ${membre.username}.`
      : `${membre.username} a déjà un salon : <#${result.channelId}>.`,
  );
}

export async function handleProspectAvis(interaction: ChatInputCommandInteraction) {
  const context = await requireGrade(interaction);
  if (!context) {
    return;
  }

  const membre = interaction.options.getUser("membre", true);
  const entry = await findEntryInCurrentSession(context.guild.id, membre.id);
  if (!entry) {
    await interaction.reply({
      content: `${membre.username} ne figure pas dans la liste des prospects de la semaine.`,
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const embed = new EmbedBuilder()
    .setTitle(`Avis sur ${membre.username}`)
    .setColor(entry.vetoById ? VETO_COLOR : PROSPECT_COLOR)
    .setDescription(`${mention(entry.userId)}\n${formatTally(entry.votes)}`)
    .addFields({ name: "Détail", value: formatEntryDetail(entry) })
    .setFooter({ text: `Semaine du ${formatDateLabel(entry.session.weekStart).slice(0, 10)}` });

  await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
}

export async function handleProspectHistorique(interaction: ChatInputCommandInteraction) {
  const context = await requireGrade(interaction);
  if (!context) {
    return;
  }

  const membre = interaction.options.getUser("membre", true);
  const entries = await getUserHistory(context.guild.id, membre.id);
  if (entries.length === 0) {
    await interaction.reply({
      content: `Aucun historique d'avis pour ${membre.username}.`,
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const embed = new EmbedBuilder()
    .setTitle(`Historique des avis — ${membre.username}`)
    .setColor(PROSPECT_COLOR)
    .addFields(
      entries.map((entry) => ({
        name: `Semaine du ${formatDateLabel(entry.session.weekStart).slice(0, 10)}${entry.vetoById ? " ⛔" : ""}`,
        value: truncateField(
          `${formatTally(entry.votes)}${entry.vetoMotif ? `\n⛔ ${entry.vetoMotif}` : ""}`,
        ),
      })),
    );

  await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
}

export async function handleProspectReset(interaction: ChatInputCommandInteraction) {
  if (!interaction.guild || !interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }
  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageGuild)) {
    await interaction.reply(MANAGE_GUILD_ONLY);
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  const session = await ensureCurrentSession(interaction.guild);
  await syncProspectDashboard(interaction.client, interaction.guildId);
  await interaction.editReply(
    `Semaine en cours : ${session.entries.length} prospect(s). Les semaines précédentes restent consultables via \`/prospect historique\`.`,
  );
}

export async function handleProspectVetoPoser(interaction: ChatInputCommandInteraction) {
  if (!interaction.guild || !interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }
  const config = await getProspectConfig(interaction.guildId);
  if (!isLead(interaction.member, config)) {
    await interaction.reply({ content: NOT_LEAD, flags: MessageFlags.Ephemeral });
    return;
  }

  const membre = interaction.options.getUser("membre", true);
  const motif = interaction.options.getString("motif", true).trim();

  const entry = await findEntryInCurrentSession(interaction.guildId, membre.id);
  if (!entry) {
    await interaction.reply({
      content: `${membre.username} ne figure pas dans la liste des prospects de la semaine.`,
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  await setVeto(entry.id, interaction.user.id, motif);
  await syncProspectDashboard(interaction.client, interaction.guildId);
  await interaction.editReply(`⛔ Véto posé sur ${membre.username}.`);
}

export async function handleProspectVetoLever(interaction: ChatInputCommandInteraction) {
  if (!interaction.guild || !interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }
  const config = await getProspectConfig(interaction.guildId);
  if (!isLead(interaction.member, config)) {
    await interaction.reply({ content: NOT_LEAD, flags: MessageFlags.Ephemeral });
    return;
  }

  const membre = interaction.options.getUser("membre", true);
  const entry = await findEntryInCurrentSession(interaction.guildId, membre.id);
  if (!entry?.vetoById) {
    await interaction.reply({
      content: `Aucun véto actif sur ${membre.username}.`,
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  await clearVeto(entry.id);
  await syncProspectDashboard(interaction.client, interaction.guildId);
  await interaction.editReply(`Véto levé sur ${membre.username}.`);
}
