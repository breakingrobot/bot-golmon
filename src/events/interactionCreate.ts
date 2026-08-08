import {
  ActionRowBuilder,
  ModalBuilder,
  TextInputBuilder,
  TextInputStyle,
  type Interaction,
} from "discord.js";
import { ResponseStatus } from "@prisma/client";
import type { BotEvent } from "../types/event.js";
import { decodeCustomId, encodeCustomId } from "../lib/customId.js";
import { logger } from "../lib/logger.js";
import {
  getNonRespondentIds,
  getPresenceWithResponses,
  registerResponse,
  syncPresenceMessage,
} from "../modules/presences/service.js";

async function handleChatInputCommand(interaction: Interaction) {
  if (!interaction.isChatInputCommand()) {
    return;
  }
  const command = interaction.client.commands.get(interaction.commandName);
  if (!command) {
    return;
  }
  try {
    await command.execute(interaction);
  } catch (error) {
    logger.error(`Erreur lors de l'exécution de /${interaction.commandName}`, error);
    const payload = { content: "Une erreur est survenue.", ephemeral: true } as const;
    if (interaction.replied || interaction.deferred) {
      await interaction.followUp(payload);
    } else {
      await interaction.reply(payload);
    }
  }
}

async function handleAutocomplete(interaction: Interaction) {
  if (!interaction.isAutocomplete()) {
    return;
  }
  const command = interaction.client.commands.get(interaction.commandName);
  if (!command?.autocomplete) {
    return;
  }
  try {
    await command.autocomplete(interaction);
  } catch (error) {
    logger.error(`Erreur d'autocomplete pour /${interaction.commandName}`, error);
  }
}

async function handlePresenceButton(interaction: Interaction) {
  if (!interaction.isButton()) {
    return;
  }

  const { namespace, action, id } = decodeCustomId(interaction.customId);
  if (namespace !== "presence") {
    return;
  }

  const presence = await getPresenceWithResponses(id);
  if (!presence) {
    await interaction.reply({ content: "Présence introuvable.", ephemeral: true });
    return;
  }
  if (presence.closedAt) {
    await interaction.reply({ content: "Cette présence est clôturée.", ephemeral: true });
    return;
  }

  if (action === "present" || action === "absent") {
    await registerResponse(
      id,
      interaction.user.id,
      action === "present" ? ResponseStatus.PRESENT : ResponseStatus.ABSENT,
    );
    await interaction.deferUpdate();
    await syncPresenceMessage(interaction.client, id);
    return;
  }

  if (action === "retard") {
    const modal = new ModalBuilder()
      .setCustomId(encodeCustomId("presence", "retard-modal", id))
      .setTitle("Retard")
      .addComponents(
        new ActionRowBuilder<TextInputBuilder>().addComponents(
          new TextInputBuilder()
            .setCustomId("minutes")
            .setLabel("Durée du retard (minutes)")
            .setStyle(TextInputStyle.Short)
            .setRequired(true),
        ),
      );
    await interaction.showModal(modal);
    return;
  }

  if (action === "ping") {
    await interaction.deferReply({ ephemeral: true });
    if (!interaction.guild) {
      await interaction.editReply("Commande utilisable en serveur uniquement.");
      return;
    }
    const nonRespondentIds = await getNonRespondentIds(interaction.guild, presence);
    if (nonRespondentIds.length === 0) {
      await interaction.editReply("Tout le monde a répondu.");
      return;
    }
    if (interaction.channel?.isSendable()) {
      await interaction.channel.send({
        content: `Rappel présence : ${nonRespondentIds.map((userId) => `<@${userId}>`).join(" ")}`,
        allowedMentions: { parse: ["users"] },
      });
    }
    await interaction.editReply(`${nonRespondentIds.length} membre(s) rappelé(s).`);
  }
}

async function handlePresenceModal(interaction: Interaction) {
  if (!interaction.isModalSubmit()) {
    return;
  }

  const { namespace, action, id } = decodeCustomId(interaction.customId);
  if (namespace !== "presence" || action !== "retard-modal") {
    return;
  }

  const minutesRaw = interaction.fields.getTextInputValue("minutes");
  const minutes = Number.parseInt(minutesRaw, 10);
  if (Number.isNaN(minutes) || minutes < 0) {
    await interaction.reply({ content: "Durée invalide.", ephemeral: true });
    return;
  }

  const presence = await getPresenceWithResponses(id);
  if (!presence || presence.closedAt) {
    await interaction.reply({ content: "Présence introuvable ou clôturée.", ephemeral: true });
    return;
  }

  await registerResponse(id, interaction.user.id, ResponseStatus.RETARD, minutes);
  await interaction.deferUpdate();
  await syncPresenceMessage(interaction.client, id);
}

export const event: BotEvent<"interactionCreate"> = {
  name: "interactionCreate",
  async execute(interaction) {
    await handleChatInputCommand(interaction);
    await handleAutocomplete(interaction);
    await handlePresenceButton(interaction);
    await handlePresenceModal(interaction);
  },
};
