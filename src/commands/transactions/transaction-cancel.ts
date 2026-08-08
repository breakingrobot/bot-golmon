import { SlashCommandBuilder } from "discord.js";
import { TransactionStatus } from "@prisma/client";
import type { Command } from "../../types/command.js";
import { findOpenTransactions, setTransactionStatus, syncTransactionMessage } from "../../modules/transactions/service.js";

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("transaction-cancel")
    .setDescription("Annule une transaction")
    .addStringOption((option) =>
      option
        .setName("transaction_id")
        .setDescription("Transaction concernée")
        .setRequired(true)
        .setAutocomplete(true),
    ),
  async execute(interaction) {
    const transactionId = interaction.options.getString("transaction_id", true);

    await setTransactionStatus(transactionId, TransactionStatus.ANNULEE);
    await syncTransactionMessage(interaction.client, transactionId);

    await interaction.reply({ content: "Transaction annulée.", ephemeral: true });
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
