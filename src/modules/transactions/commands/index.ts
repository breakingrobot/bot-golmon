import {
  EmbedBuilder,
  MessageFlags,
  PermissionFlagsBits,
  type AutocompleteInteraction,
  type ChatInputCommandInteraction,
} from "discord.js";
import { TransactionStatus, type TransactionType } from "@prisma/client";
import { buildTransactionEmbed } from "../../../lib/embeds/transactionEmbed.js";
import {
  addTransactionItem,
  createTransaction,
  findTransactions,
  getTransaction,
  getTransactionConfig,
  refreshTransaction,
  setTransactionStatus,
  syncTransactionDashboard,
  upsertTransactionConfig,
} from "../service.js";
import { TRANSACTION_STATUS_LABELS, formatPrice } from "../constants.js";

const GUILD_ONLY = {
  content: "Commande utilisable en serveur uniquement.",
  flags: MessageFlags.Ephemeral,
} as const;

export async function autocompleteTransaction(interaction: AutocompleteInteraction) {
  if (!interaction.guildId) {
    await interaction.respond([]);
    return;
  }
  // `view` doit pouvoir retrouver aussi les transactions terminées.
  const openOnly = interaction.options.getSubcommand() !== "view";
  const transactions = await findTransactions(
    interaction.guildId,
    interaction.options.getFocused(),
    openOnly,
  );
  await interaction.respond(
    transactions.map((transaction) => ({
      name: `${transaction.groupe} · ${formatPrice(transaction.prix)} · ${TRANSACTION_STATUS_LABELS[transaction.statut]}`.slice(0, 100),
      value: transaction.id,
    })),
  );
}

export async function handleTransactionConfig(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const dashboardChannel = interaction.options.getChannel("dashboard_channel");

  if (!dashboardChannel) {
    const config = await getTransactionConfig(interaction.guildId);
    const embed = new EmbedBuilder()
      .setTitle("Configuration des transactions")
      .setColor(0x5865f2)
      .addFields({
        name: "Salon du tableau de bord",
        value: config?.dashboardChannelId ? `<#${config.dashboardChannelId}>` : "*non configuré*",
      });
    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
    return;
  }

  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageGuild)) {
    await interaction.reply({
      content: "Seuls les membres avec la permission « Gérer le serveur » peuvent modifier la configuration.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  // Nouveau salon => l'ancien message de tableau de bord n'est plus pertinent.
  await upsertTransactionConfig({
    guildId: interaction.guildId,
    dashboardChannelId: dashboardChannel.id,
    dashboardMessageId: null,
  });
  await syncTransactionDashboard(interaction.client, interaction.guildId);
  await interaction.editReply(`Tableau de bord installé dans <#${dashboardChannel.id}>.`);
}

export async function handleTransactionCreate(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const transaction = await createTransaction({
    guildId: interaction.guildId,
    // L'embed détaillé reste dans le salon de travail ; le tableau de bord
    // (salon configuré) donne la vue d'ensemble.
    channelId: interaction.channelId,
    groupe: interaction.options.getString("groupe", true),
    type: interaction.options.getString("type", true) as TransactionType,
    prix: interaction.options.getNumber("prix", true),
    createdById: interaction.user.id,
  });

  await refreshTransaction(interaction.client, transaction.id, interaction.guildId);
  await interaction.editReply(
    `Transaction créée pour **${transaction.groupe}**. Ajoute des objets avec \`/transaction add-item\`.`,
  );
}

export async function handleTransactionAddItem(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const transactionId = interaction.options.getString("transaction", true);
  const nom = interaction.options.getString("nom", true);

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  await addTransactionItem(transactionId, {
    nom,
    quantite: interaction.options.getInteger("quantite") ?? 1,
    prixUnitaire: interaction.options.getNumber("prix_unitaire") ?? undefined,
  });

  await refreshTransaction(interaction.client, transactionId, interaction.guildId);
  await interaction.editReply(`Objet « ${nom} » ajouté.`);
}

export async function handleTransactionStatus(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const transactionId = interaction.options.getString("transaction", true);
  const statut = interaction.options.getString("statut", true) as TransactionStatus;

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  await setTransactionStatus(transactionId, statut);
  await refreshTransaction(interaction.client, transactionId, interaction.guildId);
  await interaction.editReply(`Statut passé à **${TRANSACTION_STATUS_LABELS[statut]}**.`);
}

export async function handleTransactionCancel(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const transactionId = interaction.options.getString("transaction", true);

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });
  await setTransactionStatus(transactionId, TransactionStatus.ANNULEE);
  await refreshTransaction(interaction.client, transactionId, interaction.guildId);
  await interaction.editReply("Transaction annulée.");
}

export async function handleTransactionView(interaction: ChatInputCommandInteraction) {
  const transaction = await getTransaction(interaction.options.getString("transaction", true));
  if (!transaction) {
    await interaction.reply({ content: "Transaction introuvable.", flags: MessageFlags.Ephemeral });
    return;
  }

  await interaction.reply({
    embeds: [buildTransactionEmbed(transaction)],
    flags: MessageFlags.Ephemeral,
  });
}
