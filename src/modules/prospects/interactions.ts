import {
  EmbedBuilder,
  LabelBuilder,
  MessageFlags,
  ModalBuilder,
  StringSelectMenuBuilder,
  TextInputBuilder,
  TextInputStyle,
  type ModalSubmitInteraction,
  type StringSelectMenuInteraction,
} from "discord.js";
import { encodeCustomId } from "../../lib/customId.js";
import { mention } from "../../lib/format.js";
import { formatEntryDetail, formatTally } from "../../lib/embeds/prospectDashboardEmbed.js";
import { VOTE_SELECT_OPTIONS } from "../../lib/components/prospectComponents.js";
import {
  MOTIF_MAX_LENGTH,
  PROSPECT_COLOR,
  PROSPECT_NAMESPACE,
  VETO_COLOR,
  VOTE_EMOJIS,
  VOTE_LABELS,
  isVoteChoice,
} from "./constants.js";
import { isGrade, isLead, NOT_GRADE, NOT_LEAD } from "./permissions.js";
import {
  castVote,
  getEntry,
  getProspectConfig,
  setVeto,
  syncProspectDashboard,
} from "./service.js";

const ENTRY_GONE = {
  content: "Ce prospect n'est plus suivi (la semaine a peut-être été réinitialisée).",
  flags: MessageFlags.Ephemeral,
} as const;

/**
 * Modal du vote : un select « Pour / Contre / Blanc » et un motif obligatoire.
 *
 * Les select menus dans une modal passent par des composants `Label`
 * (`addLabelComponents`) ; `addComponents` est déprécié sur `ModalBuilder` dans
 * cette version de discord.js et ne gère que les champs texte.
 */
function buildVoteModal(entryId: string, displayName: string): ModalBuilder {
  return new ModalBuilder()
    .setCustomId(encodeCustomId(PROSPECT_NAMESPACE, "vote-modal", entryId))
    .setTitle(`Avis sur ${displayName}`.slice(0, 45))
    .addLabelComponents(
      new LabelBuilder()
        .setLabel("Vote")
        .setStringSelectMenuComponent(
          new StringSelectMenuBuilder()
            .setCustomId("choix")
            .setPlaceholder("Choisis ton vote")
            .setRequired(true)
            .addOptions(VOTE_SELECT_OPTIONS),
        ),
      new LabelBuilder()
        .setLabel("Motif du vote")
        .setDescription("Explique pourquoi — visible par les gradés.")
        .setTextInputComponent(
          new TextInputBuilder()
            .setCustomId("motif")
            .setStyle(TextInputStyle.Paragraph)
            .setRequired(true)
            .setMaxLength(MOTIF_MAX_LENGTH),
        ),
    );
}

function buildVetoModal(entryId: string, displayName: string): ModalBuilder {
  return new ModalBuilder()
    .setCustomId(encodeCustomId(PROSPECT_NAMESPACE, "veto-modal", entryId))
    .setTitle(`Véto sur ${displayName}`.slice(0, 45))
    .addLabelComponents(
      new LabelBuilder()
        .setLabel("Motif du véto")
        .setDescription("Obligatoire — le véto bloque la validation du prospect.")
        .setTextInputComponent(
          new TextInputBuilder()
            .setCustomId("motif")
            .setStyle(TextInputStyle.Paragraph)
            .setRequired(true)
            .setMaxLength(MOTIF_MAX_LENGTH),
        ),
    );
}

/** Nom affichable d'un prospect pour les titres de modal (45 caractères max). */
async function displayNameOf(
  interaction: StringSelectMenuInteraction,
  userId: string,
): Promise<string> {
  const member = await interaction.guild?.members.fetch(userId).catch(() => null);
  return member?.displayName ?? "ce prospect";
}

export async function handleProspectSelect(interaction: StringSelectMenuInteraction, action: string) {
  if (!interaction.guildId) {
    return;
  }

  const entryId = interaction.values[0];
  if (!entryId) {
    return;
  }

  const config = await getProspectConfig(interaction.guildId);
  const entry = await getEntry(entryId);
  if (!entry) {
    await interaction.reply(ENTRY_GONE);
    return;
  }

  if (action === "detail") {
    if (!isGrade(interaction.member, config)) {
      await interaction.reply({ content: NOT_GRADE, flags: MessageFlags.Ephemeral });
      return;
    }
    const embed = new EmbedBuilder()
      .setTitle("Avis détaillés")
      .setColor(entry.vetoById ? VETO_COLOR : PROSPECT_COLOR)
      .setDescription(`${mention(entry.userId)}\n${formatTally(entry.votes)}`)
      .addFields({ name: "Détail", value: formatEntryDetail(entry) });
    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
    return;
  }

  if (action === "vote") {
    if (!isGrade(interaction.member, config)) {
      await interaction.reply({ content: NOT_GRADE, flags: MessageFlags.Ephemeral });
      return;
    }
    if (entry.session.closedAt) {
      await interaction.reply({
        content: "Cette semaine d'avis est clôturée.",
        flags: MessageFlags.Ephemeral,
      });
      return;
    }
    await interaction.showModal(buildVoteModal(entryId, await displayNameOf(interaction, entry.userId)));
    return;
  }

  if (action === "veto") {
    if (!isLead(interaction.member, config)) {
      await interaction.reply({ content: NOT_LEAD, flags: MessageFlags.Ephemeral });
      return;
    }
    if (entry.vetoById) {
      await interaction.reply({
        content: `Ce prospect est déjà sous véto de ${mention(entry.vetoById)}. Utilise \`/prospect veto lever\` pour le retirer.`,
        flags: MessageFlags.Ephemeral,
      });
      return;
    }
    await interaction.showModal(buildVetoModal(entryId, await displayNameOf(interaction, entry.userId)));
  }
}

export async function handleProspectModal(
  interaction: ModalSubmitInteraction,
  action: string,
  entryId: string,
) {
  if (!interaction.guildId) {
    return;
  }

  const config = await getProspectConfig(interaction.guildId);
  const entry = await getEntry(entryId);
  if (!entry) {
    await interaction.reply(ENTRY_GONE);
    return;
  }

  const motif = interaction.fields.getTextInputValue("motif").trim();
  if (!motif) {
    await interaction.reply({
      content: "Le motif est obligatoire.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  if (action === "vote-modal") {
    if (!isGrade(interaction.member, config)) {
      await interaction.reply({ content: NOT_GRADE, flags: MessageFlags.Ephemeral });
      return;
    }

    const choix = interaction.fields.getStringSelectValues("choix")[0];
    if (!choix || !isVoteChoice(choix)) {
      await interaction.reply({ content: "Vote invalide.", flags: MessageFlags.Ephemeral });
      return;
    }

    await castVote(entryId, interaction.user.id, choix, motif);
    await interaction.deferUpdate();
    await syncProspectDashboard(interaction.client, interaction.guildId);
    await interaction.followUp({
      content: `${VOTE_EMOJIS[choix]} Avis « ${VOTE_LABELS[choix]} » enregistré pour ${mention(entry.userId)}.${
        entry.vetoById ? "\n⛔ Attention : ce prospect est sous véto." : ""
      }`,
      flags: MessageFlags.Ephemeral,
      allowedMentions: { parse: [] },
    });
    return;
  }

  if (action === "veto-modal") {
    if (!isLead(interaction.member, config)) {
      await interaction.reply({ content: NOT_LEAD, flags: MessageFlags.Ephemeral });
      return;
    }

    await setVeto(entryId, interaction.user.id, motif);
    await interaction.deferUpdate();
    await syncProspectDashboard(interaction.client, interaction.guildId);
    await interaction.followUp({
      content: `⛔ Véto posé sur ${mention(entry.userId)}.`,
      flags: MessageFlags.Ephemeral,
      allowedMentions: { parse: [] },
    });
  }
}
