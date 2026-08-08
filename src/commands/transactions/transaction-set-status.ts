import { SlashCommandBuilder } from "discord.js";
import type { TransactionStatus } from "@prisma/client";
import type { Command } from "../../types/command.js";
import { TRANSACTION_STATUS_CHOICES } from "../../modules/transactions/constants.js";
import { findOpenTransactions, setTransactionStatus, syncTransactionMessage } from "../../modules/transactions/service.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("transaction-set-status")
    .setDescription("Change le statut d'une transaction")
    .addStringOption((option) =>
      option
        .setName("transaction_id")
        .setDescription("Transaction concernée")
        .setRequired(true)
        .setAutocomplete(true),
    )
    .addStringOption((option) =>
      option
        .setName("statut")
        .setDescription("Nouveau statut")
        .setRequired(true)
        .addChoices(...TRANSACTION_STATUS_CHOICES),
    ),
  async execute(interaction) {
    const transactionId = interaction.options.getString("transaction_id", true);
    const statut = interaction.options.getString("statut", true) as TransactionStatus;

    await setTransactionStatus(transactionId, statut);
    await syncTransactionMessage(interaction.client, transactionId);

    await interaction.reply({ content: "Statut mis à jour.", ephemeral: true });
  },
  async autocomplete(interaction) {
    if (!interaction.guildId) {
      await interaction.respond([]);
      return;
    }
    const query = interaction.options.getFocused();
    const transactions = await findOpenTransactions(interaction.guildId, query);
    await interaction.respond(
      transactions.map((t) => ({ name: `${t.groupe} — ${t.id}`, value: t.id })),
    );
  },
};
