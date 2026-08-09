import { ChannelType, SlashCommandBuilder, type SlashCommandStringOption } from "discord.js";
import type { Command } from "../types/command.js";
import {
  TRANSACTION_STATUS_CHOICES,
  TRANSACTION_TYPE_CHOICES,
} from "../modules/transactions/constants.js";
import {
  autocompleteTransaction,
  handleTransactionAddItem,
  handleTransactionCancel,
  handleTransactionConfig,
  handleTransactionCreate,
  handleTransactionStatus,
  handleTransactionView,
  handleTransactionDelete,
} from "../modules/transactions/commands/index.js";

function transactionOption(option: SlashCommandStringOption) {
  return option
    .setName("transaction")
    .setDescription("Transaction concernée")
    .setRequired(true)
    .setAutocomplete(true);
}

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("transaction")
    .setDescription("Gestion des transactions du groupe")
    .addSubcommand((sub) =>
      sub
        .setName("create")
        .setDescription("Crée une transaction")
        .addStringOption((option) =>
          option.setName("groupe").setDescription("Groupe concerné").setRequired(true),
        )
        .addStringOption((option) =>
          option
            .setName("type")
            .setDescription("Achat ou vente")
            .setRequired(true)
            .addChoices(...TRANSACTION_TYPE_CHOICES),
        )
        .addNumberOption((option) =>
          option.setName("prix").setDescription("Prix total").setRequired(true),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("add-item")
        .setDescription("Ajoute un objet à une transaction")
        .addStringOption(transactionOption)
        .addStringOption((option) =>
          option.setName("nom").setDescription("Nom de l'objet").setRequired(true),
        )
        .addIntegerOption((option) =>
          option.setName("quantite").setDescription("Quantité (défaut: 1)").setMinValue(1),
        )
        .addNumberOption((option) =>
          option.setName("prix_unitaire").setDescription("Prix unitaire"),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("status")
        .setDescription("Change le statut d'une transaction")
        .addStringOption(transactionOption)
        .addStringOption((option) =>
          option
            .setName("statut")
            .setDescription("Nouveau statut")
            .setRequired(true)
            .addChoices(...TRANSACTION_STATUS_CHOICES),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("view")
        .setDescription("Affiche une transaction")
        .addStringOption(transactionOption),
    )
    .addSubcommand((sub) =>
      sub
        .setName("cancel")
        .setDescription("Annule une transaction")
        .addStringOption(transactionOption),
    )
    .addSubcommand((sub) =>
      sub
        .setName("config")
        .setDescription("Configure le tableau de bord (sans option: affiche la config)")
        .addChannelOption((option) =>
          option
            .setName("dashboard_channel")
            .setDescription("Salon du tableau de bord permanent")
            .addChannelTypes(ChannelType.GuildText),
        )
        .addRoleOption((option) =>
          option
            .setName("ping_role")
            .setDescription("Rôle à mentionner lors de la création d'une transaction"),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("delete")
        .setDescription("Supprime définitivement une transaction de la base de données")
        .addStringOption(transactionOption),
    ),

  async execute(interaction) {
    switch (interaction.options.getSubcommand()) {
      case "create":
        return handleTransactionCreate(interaction);
      case "add-item":
        return handleTransactionAddItem(interaction);
      case "status":
        return handleTransactionStatus(interaction);
      case "view":
        return handleTransactionView(interaction);
      case "cancel":
        return handleTransactionCancel(interaction);
      case "delete":
        return handleTransactionDelete(interaction);
      case "config":
        return handleTransactionConfig(interaction);
      default:
        return;
    }
  },

  async autocomplete(interaction) {
    return autocompleteTransaction(interaction);
  },
};
