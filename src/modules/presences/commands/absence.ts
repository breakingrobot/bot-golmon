import {
  EmbedBuilder,
  MessageFlags,
  PermissionFlagsBits,
  type ChatInputCommandInteraction,
} from "discord.js";
import { parseEventDate, todayLabel } from "../../../lib/datetime.js";
import { truncateField } from "../../../lib/format.js";
import { logger } from "../../../lib/logger.js";
import {
  declareAbsence,
  endAbsence,
  getActiveAbsence,
  listActiveAbsences,
} from "../absences.js";
import { getPresenceConfig } from "../service.js";

const GUILD_ONLY = {
  content: "Commande utilisable en serveur uniquement.",
  flags: MessageFlags.Ephemeral,
} as const;

function unix(date: Date): number {
  return Math.floor(date.getTime() / 1000);
}

/** Une date seule vaut le jour entier : début à 00:00, fin à 23:59. */
function parseDayBoundary(label: string, boundary: "start" | "end"): Date | null {
  return parseEventDate(label, boundary === "start" ? "00:00" : "23:59");
}

async function announceAbsence(
  interaction: ChatInputCommandInteraction,
  guildId: string,
  embed: EmbedBuilder,
): Promise<void> {
  const config = await getPresenceConfig(guildId);
  if (!config?.absenceChannelId) {
    return;
  }

  try {
    const channel = await interaction.client.channels.fetch(config.absenceChannelId).catch((err) => {
      logger.warn(`Impossible de récupérer le salon des absences : ${err.message}`);
      return null;
    });
    if (channel?.isSendable()) {
      await channel.send({ embeds: [embed] });
    }
  } catch (error) {
    logger.warn(`Annonce d'absence impossible: ${String(error)}`);
  }
}

export async function handleAbsenceDeclare(interaction: ChatInputCommandInteraction) {
  if (!interaction.guild) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const targetOption = interaction.options.getUser("membre");
  const isForSomeoneElse = targetOption !== null && targetOption.id !== interaction.user.id;

  if (isForSomeoneElse && !interaction.memberPermissions?.has(PermissionFlagsBits.ManageMessages)) {
    await interaction.reply({
      content: "Seul le staff peut déclarer l'absence d'un autre membre.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const target = targetOption ?? interaction.user;
  const motif = interaction.options.getString("motif", true);

  const startsAt = parseDayBoundary(interaction.options.getString("debut") ?? todayLabel(), "start");
  const endsAt = parseDayBoundary(interaction.options.getString("fin", true), "end");

  if (!startsAt || !endsAt) {
    await interaction.reply({
      content: "Date invalide. Format attendu : `JJ/MM/AAAA`.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  if (endsAt.getTime() <= startsAt.getTime()) {
    await interaction.reply({
      content: "La date de fin doit être postérieure à la date de début.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const { roleResult, affected } = await declareAbsence(interaction.client, interaction.guild, {
    userId: target.id,
    motif,
    startsAt,
    endsAt,
    createdById: interaction.user.id,
  });

  const embed = new EmbedBuilder()
    .setTitle("🌙 Absence déclarée")
    .setColor(0xe67e22)
    .setThumbnail(target.displayAvatarURL())
    .addFields(
      { name: "Membre", value: `<@${target.id}>`, inline: true },
      { name: "Du", value: `<t:${unix(startsAt)}:d>`, inline: true },
      { name: "Au", value: `<t:${unix(endsAt)}:d>`, inline: true },
      { name: "Motif", value: truncateField(motif) },
    );

  if (isForSomeoneElse) {
    embed.setFooter({ text: `Déclarée par ${interaction.user.username}` });
  }

  await announceAbsence(interaction, interaction.guild.id, embed);

  // Le rôle peut échouer (permission ou hiérarchie) sans invalider l'absence.
  const roleNote = roleResult.ok
    ? ""
    : `\n⚠️ Rôle d'absence non appliqué : ${roleResult.reason ?? "raison inconnue"}.`;
  const presenceNote =
    affected > 0 ? `\n${affected} présence(s) ouverte(s) mise(s) à jour en absent.` : "";

  await interaction.editReply({
    content: `Absence enregistrée jusqu'au <t:${unix(endsAt)}:d>.${presenceNote}${roleNote}`,
    embeds: [embed],
  });
}

export async function handleAbsenceEnd(interaction: ChatInputCommandInteraction) {
  if (!interaction.guild) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const targetOption = interaction.options.getUser("membre");
  const isForSomeoneElse = targetOption !== null && targetOption.id !== interaction.user.id;

  if (isForSomeoneElse && !interaction.memberPermissions?.has(PermissionFlagsBits.ManageMessages)) {
    await interaction.reply({
      content: "Seul le staff peut clôturer l'absence d'un autre membre.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const target = targetOption ?? interaction.user;
  const absence = await getActiveAbsence(interaction.guild.id, target.id);

  if (!absence) {
    await interaction.reply({
      content: `Aucune absence en cours pour <@${target.id}>.`,
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const result = await endAbsence(interaction.guild, absence.id, target.id);
  const note = result.ok
    ? ""
    : `\n⚠️ Rôle non retiré : ${result.reason ?? "raison inconnue"}.`;

  await interaction.editReply(`Absence de <@${target.id}> clôturée.${note}`);
}

export async function handleAbsenceList(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const absences = await listActiveAbsences(interaction.guildId);

  const embed = new EmbedBuilder()
    .setTitle("🌙 Absences en cours")
    .setColor(0xe67e22)
    .setDescription(
      absences.length === 0
        ? "Personne n'est absent actuellement."
        : truncateField(
            absences
              .map(
                (absence) =>
                  `• <@${absence.userId}> — jusqu'au <t:${unix(absence.endsAt)}:d> — ${absence.motif}`,
              )
              .join("\n"),
            4096,
          ),
    )
    .setFooter({ text: `${absences.length} absence(s)` });

  await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
}
