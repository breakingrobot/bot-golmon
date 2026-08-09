import {
  ActionRowBuilder,
  MessageFlags,
  ModalBuilder,
  TextInputBuilder,
  TextInputStyle,
  PermissionFlagsBits,
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
import { addTransactionItem, refreshTransaction, setTransactionStatus, deleteTransaction, syncTransactionDashboard } from "../modules/transactions/service.js";
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
  if (action === "additem") {
    await interaction.showModal(
      new ModalBuilder()
        .setCustomId(encodeCustomId("transaction", "additem-modal", id))
        .setTitle("Ajouter un objet")
        .addComponents(
          new ActionRowBuilder<TextInputBuilder>().addComponents(
            new TextInputBuilder()
              .setCustomId("nom")
              .setLabel("Nom de l'objet")
              .setPlaceholder("Arme, Voiture, etc.")
              .setStyle(TextInputStyle.Short)
              .setRequired(true),
          ),
          new ActionRowBuilder<TextInputBuilder>().addComponents(
            new TextInputBuilder()
              .setCustomId("quantite")
              .setLabel("Quantité")
              .setPlaceholder("1")
              .setStyle(TextInputStyle.Short)
              .setRequired(false),
          ),
          new ActionRowBuilder<TextInputBuilder>().addComponents(
            new TextInputBuilder()
              .setCustomId("prix_unitaire")
              .setLabel("Prix unitaire (optionnel)")
              .setPlaceholder("150000")
              .setStyle(TextInputStyle.Short)
              .setRequired(false),
          ),
        ),
    );
    return;
  }

  if (action === "delete") {
    if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageMessages)) {
      await interaction.reply({
        content: "Tu dois avoir la permission de gérer les messages pour supprimer une transaction.",
        flags: MessageFlags.Ephemeral,
      });
      return;
    }
    
    await interaction.deferUpdate();
    
    if (interaction.message?.deletable) {
      await interaction.message.delete().catch(() => null);
    }
    await deleteTransaction(id).catch(() => null);
    if (interaction.guildId) {
      await syncTransactionDashboard(interaction.client, interaction.guildId);
    }
    await interaction.followUp({ content: "Transaction supprimée.", flags: MessageFlags.Ephemeral });
    return;
  }

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

async function handleTransactionAddItemModal(interaction: ModalSubmitInteraction, id: string) {
  const nom = interaction.fields.getTextInputValue("nom");
  const quantiteStr = interaction.fields.getTextInputValue("quantite");
  const prixUnitaireStr = interaction.fields.getTextInputValue("prix_unitaire");

  const quantite = quantiteStr ? parseInt(quantiteStr, 10) : 1;
  const prixUnitaire = prixUnitaireStr ? parseFloat(prixUnitaireStr.replace(",", ".")) : undefined;

  if (isNaN(quantite) || quantite < 1) {
    await interaction.reply({ content: "Quantité invalide.", flags: MessageFlags.Ephemeral });
    return;
  }
  if (prixUnitaire !== undefined && isNaN(prixUnitaire)) {
    await interaction.reply({ content: "Prix unitaire invalide.", flags: MessageFlags.Ephemeral });
    return;
  }

  if (!interaction.guildId) return;

  await interaction.deferUpdate();
  
  try {
    await addTransactionItem(id, { nom, quantite, prixUnitaire });
    await refreshTransaction(interaction.client, id, interaction.guildId);
    await interaction.followUp({ content: `Objet « ${nom} » ajouté.`, flags: MessageFlags.Ephemeral });
  } catch (err: any) {
    if (err.code === "P2003" || err.code === "P2025") {
      await interaction.followUp({ content: "Transaction introuvable (elle a peut-être été supprimée).", flags: MessageFlags.Ephemeral });
    } else {
      throw err;
    }
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
    } else if (parsed?.namespace === "transaction" && parsed.action === "additem-modal") {
      await handleTransactionAddItemModal(interaction, parsed.id);
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
