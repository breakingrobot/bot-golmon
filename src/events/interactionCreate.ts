import {
  ActionRowBuilder,
  MessageFlags,
  ModalBuilder,
  TextInputBuilder,
  TextInputStyle,
  type ButtonInteraction,
  type Interaction,
  type ModalSubmitInteraction,
} from "discord.js";
import { ResponseStatus, TransactionStatus } from "@prisma/client";
import type { BotEvent } from "../types/event.js";
import { decodeCustomId, encodeCustomId } from "../lib/customId.js";
import { chunkMentions } from "../lib/format.js";
import { logger } from "../lib/logger.js";
import {
  getNonRespondentIds,
  getPresenceWithResponses,
  registerResponse,
  syncPresenceMessage,
} from "../modules/presences/service.js";
import { refreshTransaction, setTransactionStatus } from "../modules/transactions/service.js";
import { TRANSACTION_STATUS_LABELS } from "../modules/transactions/constants.js";

const BUTTON_STATUS: Record<string, ResponseStatus> = {
  present: ResponseStatus.PRESENT,
  absent: ResponseStatus.ABSENT,
};

async function handlePresenceButton(interaction: ButtonInteraction, action: string, id: string) {
  const presence = await getPresenceWithResponses(id);
  if (!presence) {
    await interaction.reply({ content: "Présence introuvable.", flags: MessageFlags.Ephemeral });
    return;
  }
  if (presence.closedAt) {
    await interaction.reply({
      content: "Cette présence est clôturée.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const status = BUTTON_STATUS[action];
  if (status) {
    const { changed } = await registerResponse(id, interaction.user.id, status);
    await interaction.deferUpdate();
    // Rien n'a bougé : inutile de rééditer le message.
    if (changed) {
      await syncPresenceMessage(interaction.client, id);
    }
    return;
  }

  if (action === "retard") {
    await interaction.showModal(
      new ModalBuilder()
        .setCustomId(encodeCustomId("presence", "retard-modal", id))
        .setTitle("Retard")
        .addComponents(
          new ActionRowBuilder<TextInputBuilder>().addComponents(
            new TextInputBuilder()
              .setCustomId("minutes")
              .setLabel("Durée du retard (minutes)")
              .setPlaceholder("15")
              .setStyle(TextInputStyle.Short)
              .setRequired(true),
          ),
        ),
    );
    return;
  }

  if (action === "ping") {
    await interaction.deferReply({ flags: MessageFlags.Ephemeral });
    if (!interaction.guild) {
      await interaction.editReply("Action utilisable en serveur uniquement.");
      return;
    }

    const nonRespondentIds = await getNonRespondentIds(interaction.guild, presence);
    if (nonRespondentIds.length === 0) {
      await interaction.editReply("Tout le monde a répondu.");
      return;
    }

    if (interaction.channel?.isSendable()) {
      for (const content of chunkMentions(nonRespondentIds, `⏰ Rappel — **${presence.titre}**`)) {
        await interaction.channel.send({ content, allowedMentions: { parse: ["users"] } });
      }
    }
    await interaction.editReply(`${nonRespondentIds.length} membre(s) relancé(s).`);
  }
}

async function handleTransactionButton(
  interaction: ButtonInteraction,
  action: string,
  id: string,
  payload: string | undefined,
) {
  if (action !== "status" || !payload || !interaction.guildId) {
    return;
  }

  const statut = payload as TransactionStatus;
  if (!Object.values(TransactionStatus).includes(statut)) {
    return;
  }

  await interaction.deferUpdate();
  await setTransactionStatus(id, statut);
  await refreshTransaction(interaction.client, id, interaction.guildId);
  await interaction.followUp({
    content: `Statut passé à **${TRANSACTION_STATUS_LABELS[statut]}**.`,
    flags: MessageFlags.Ephemeral,
  });
}

async function handleRetardModal(interaction: ModalSubmitInteraction, id: string) {
  const minutes = Number.parseInt(interaction.fields.getTextInputValue("minutes"), 10);
  if (Number.isNaN(minutes) || minutes < 0) {
    await interaction.reply({
      content: "Durée invalide, indique un nombre de minutes.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const presence = await getPresenceWithResponses(id);
  if (!presence || presence.closedAt) {
    await interaction.reply({
      content: "Présence introuvable ou clôturée.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const { changed } = await registerResponse(
    id,
    interaction.user.id,
    ResponseStatus.RETARD,
    minutes,
  );
  await interaction.deferUpdate();
  if (changed) {
    await syncPresenceMessage(interaction.client, id);
  }
}

async function route(interaction: Interaction) {
  if (interaction.isChatInputCommand()) {
    const command = interaction.client.commands.get(interaction.commandName);
    await command?.execute(interaction);
    return;
  }

  if (interaction.isAutocomplete()) {
    const command = interaction.client.commands.get(interaction.commandName);
    await command?.autocomplete?.(interaction);
    return;
  }

  if (interaction.isButton()) {
    const parsed = decodeCustomId(interaction.customId);
    if (!parsed) {
      return;
    }
    if (parsed.namespace === "presence") {
      await handlePresenceButton(interaction, parsed.action, parsed.id);
    } else if (parsed.namespace === "transaction") {
      await handleTransactionButton(interaction, parsed.action, parsed.id, parsed.payload);
    }
    return;
  }

  if (interaction.isModalSubmit()) {
    const parsed = decodeCustomId(interaction.customId);
    if (parsed?.namespace === "presence" && parsed.action === "retard-modal") {
      await handleRetardModal(interaction, parsed.id);
    }
  }
}

export const event: BotEvent<"interactionCreate"> = {
  name: "interactionCreate",
  async execute(interaction) {
    try {
      await route(interaction);
    } catch (error) {
      logger.error("Erreur lors du traitement d'une interaction", error);

      if (interaction.isAutocomplete() || !interaction.isRepliable()) {
        return;
      }

      const payload = {
        content: "Une erreur est survenue.",
        flags: MessageFlags.Ephemeral,
      } as const;

      try {
        if (interaction.replied || interaction.deferred) {
          await interaction.followUp(payload);
        } else {
          await interaction.reply(payload);
        }
      } catch {
        // L'interaction a expiré ou a déjà reçu une réponse : rien à faire.
      }
    }
  },
};
