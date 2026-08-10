import {
  EmbedBuilder,
  MessageFlags,
  PermissionFlagsBits,
  type AutocompleteInteraction,
  type ChatInputCommandInteraction,
} from "discord.js";
import { TransactionStatus, type TransactionType } from "@prisma/client";
import { buildTransactionEmbed } from "../../../lib/embeds/transactionEmbed.js";
import { fetchChannelSafe } from "../../../lib/discordClient.js";
import { GUILD_ONLY } from "../../../lib/replies.js";
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
  deleteTransaction,
} from "../service.js";
import { TRANSACTION_STATUS_LABELS, formatPrice } from "../constants.js";

const CLOSED = "Cette transaction est clôturée et ne peut plus être modifiée.";

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

function pingRoleUpdate(pingRole: { id: string } | null, clearPingRole: boolean) {
  if (clearPingRole) {
    return { pingRoleId: null };
  }
  return pingRole ? { pingRoleId: pingRole.id } : {};
}

export async function handleTransactionConfig(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageGuild)) {
    await interaction.reply({
      content: "Seuls les membres avec la permission « Gérer le serveur » peuvent modifier la configuration.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const dashboardChannel = interaction.options.getChannel("dashboard_channel");
  const pingRole = interaction.options.getRole("ping_role");
  const clearPingRole = interaction.options.getBoolean("clear_ping_role") ?? false;

  // Aucune option fournie : on se contente d'afficher la configuration courante.
  if (!dashboardChannel && !pingRole && !clearPingRole) {
    const config = await getTransactionConfig(interaction.guildId);
    const embed = new EmbedBuilder()
      .setTitle("Configuration des transactions")
      .setColor(0x5865f2)
      .addFields(
        {
          name: "Salon du tableau de bord",
          value: config?.dashboardChannelId ? `<#${config.dashboardChannelId}>` : "*non configuré*",
        },
        {
          name: "Rôle à ping",
          value: config?.pingRoleId ? `<@&${config.pingRoleId}>` : "*aucun*",
        },
      );
    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  await upsertTransactionConfig({
    guildId: interaction.guildId,
    // Nouveau salon => l'ancien message de tableau de bord n'est plus pertinent.
    ...(dashboardChannel
      ? { dashboardChannelId: dashboardChannel.id, dashboardMessageId: null }
      : {}),
    ...pingRoleUpdate(pingRole, clearPingRole),
  });
  await syncTransactionDashboard(interaction.client, interaction.guildId);

  const messages = [
    dashboardChannel && `Tableau de bord installé dans <#${dashboardChannel.id}>.`,
    clearPingRole && "Rôle à ping retiré.",
    !clearPingRole && pingRole && `Rôle à ping : <@&${pingRole.id}>.`,
  ].filter((part): part is string => typeof part === "string");

  await interaction.editReply(messages.join(" "));
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

  // C'est cette publication qui mentionne le rôle configuré, le cas échéant.
  await refreshTransaction(interaction.client, interaction.guildId, transaction.id);

  await interaction.editReply(`Transaction pour **${transaction.groupe}** créée avec succès.`);
}

export async function handleTransactionAddItem(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const transactionId = interaction.options.getString("transaction", true);
  const nom = interaction.options.getString("nom", true);

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const transaction = await getTransaction(interaction.guildId, transactionId);
  if (!transaction) {
    await interaction.editReply("Transaction introuvable. Assure-toi de la sélectionner correctement.");
    return;
  }

  if (transaction.statut === TransactionStatus.CLOTUREE) {
    await interaction.editReply(CLOSED);
    return;
  }

  await addTransactionItem(transactionId, {
    nom,
    quantite: interaction.options.getInteger("quantite") ?? 1,
    prixUnitaire: interaction.options.getNumber("prix_unitaire") ?? undefined,
  });

  await refreshTransaction(interaction.client, interaction.guildId, transactionId);
  await interaction.editReply(`Objet « ${nom} » ajouté.`);
}

/**
 * Volontairement ouvert à tous les membres du serveur : faire avancer une
 * transaction est une action courante du groupe, pas une action de modération.
 * Seule la suppression définitive est protégée. Ne pas « corriger » sans arbitrage.
 */
export async function handleTransactionStatus(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const transactionId = interaction.options.getString("transaction", true);
  const statut = interaction.options.getString("statut", true) as TransactionStatus;

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const transaction = await getTransaction(interaction.guildId, transactionId);
  if (!transaction) {
    await interaction.editReply("Transaction introuvable.");
    return;
  }
  if (transaction.statut === TransactionStatus.CLOTUREE) {
    await interaction.editReply(CLOSED);
    return;
  }

  await setTransactionStatus(interaction.guildId, transactionId, statut);
  await refreshTransaction(interaction.client, interaction.guildId, transactionId);
  await interaction.editReply(`Statut passé à **${TRANSACTION_STATUS_LABELS[statut]}**.`);
}

export async function handleTransactionCancel(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const transactionId = interaction.options.getString("transaction", true);
  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const transaction = await getTransaction(interaction.guildId, transactionId);
  if (!transaction) {
    await interaction.editReply("Transaction introuvable.");
    return;
  }
  if (transaction.statut === TransactionStatus.CLOTUREE) {
    await interaction.editReply(CLOSED);
    return;
  }

  await setTransactionStatus(interaction.guildId, transactionId, TransactionStatus.ANNULEE);
  await refreshTransaction(interaction.client, interaction.guildId, transactionId);
  await interaction.editReply("Transaction annulée.");
}

export async function handleTransactionDelete(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  // Seule action irréversible du module : elle reste réservée aux modérateurs.
  if (!interaction.memberPermissions?.has(PermissionFlagsBits.ManageMessages)) {
    await interaction.reply({
      content: "Tu dois avoir la permission de gérer les messages pour supprimer une transaction.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const transactionId = interaction.options.getString("transaction", true);
  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  const transaction = await getTransaction(interaction.guildId, transactionId);
  if (!transaction) {
    await interaction.editReply("Transaction introuvable.");
    return;
  }

  // Tenter de supprimer le message si possible
  if (transaction.messageId) {
    const channel = await fetchChannelSafe(
      interaction.client,
      transaction.channelId,
      `le salon de la transaction ${transactionId}`,
    );
    if (channel?.isSendable()) {
      await channel.messages.delete(transaction.messageId).catch(() => null);
    }
  }

  await deleteTransaction(interaction.guildId, transactionId);
  await syncTransactionDashboard(interaction.client, interaction.guildId);
  await interaction.editReply("Transaction supprimée définitivement de la base de données.");
}

export async function handleTransactionView(interaction: ChatInputCommandInteraction) {
  if (!interaction.guildId) {
    await interaction.reply(GUILD_ONLY);
    return;
  }

  const transaction = await getTransaction(
    interaction.guildId,
    interaction.options.getString("transaction", true),
  );
  if (!transaction) {
    await interaction.reply({ content: "Transaction introuvable.", flags: MessageFlags.Ephemeral });
    return;
  }

  await interaction.reply({
    embeds: [buildTransactionEmbed(transaction)],
    flags: MessageFlags.Ephemeral,
  });
}
