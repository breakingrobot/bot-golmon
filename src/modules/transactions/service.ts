import type { Client } from "discord.js";
import { TransactionStatus, type TransactionType } from "@prisma/client";
import { prisma } from "../../lib/prisma.js";
import { logger } from "../../lib/logger.js";
import { buildTransactionEmbed } from "../../lib/embeds/transactionEmbed.js";
import { buildTransactionDashboardEmbeds } from "../../lib/embeds/transactionDashboardEmbed.js";
import { buildTransactionButtons } from "../../lib/components/transactionButtons.js";

export async function getTransactionConfig(guildId: string) {
  return prisma.transactionConfig.findUnique({ where: { guildId } });
}

export async function upsertTransactionConfig(params: {
  guildId: string;
  dashboardChannelId?: string;
  dashboardMessageId?: string | null;
  pingRoleId?: string;
}) {
  const { guildId, ...data } = params;
  return prisma.transactionConfig.upsert({
    where: { guildId },
    create: { guildId, ...data },
    update: data,
  });
}

export async function createTransaction(params: {
  guildId: string;
  channelId: string;
  groupe: string;
  type: TransactionType;
  prix: number;
  createdById: string;
}) {
  return prisma.transaction.create({ data: params, include: { items: true } });
}

export async function addTransactionItem(
  transactionId: string,
  params: { nom: string; quantite: number; prixUnitaire?: number },
) {
  await prisma.transactionItem.create({ data: { transactionId, ...params } });
  return prisma.transaction.findUniqueOrThrow({
    where: { id: transactionId },
    include: { items: true },
  });
}

export async function setTransactionStatus(transactionId: string, statut: TransactionStatus) {
  return prisma.transaction.update({
    where: { id: transactionId },
    data: { statut },
    include: { items: true },
  });
}

export async function deleteTransaction(transactionId: string) {
  return prisma.transaction.delete({
    where: { id: transactionId },
  });
}

export async function getTransaction(transactionId: string) {
  return prisma.transaction.findUnique({
    where: { id: transactionId },
    include: { items: true },
  });
}

export async function findTransactions(guildId: string, query: string, openOnly = true) {
  return prisma.transaction.findMany({
    where: {
      guildId,
      groupe: { contains: query },
      ...(openOnly
        ? { statut: { notIn: [TransactionStatus.LIVRE_PAYE, TransactionStatus.ANNULEE] } }
        : {}),
    },
    take: 25,
    orderBy: { createdAt: "desc" },
  });
}

export async function syncTransactionMessage(client: Client, transactionId: string) {
  const transaction = await getTransaction(transactionId);
  if (!transaction) {
    return;
  }

  const channel = await client.channels.fetch(transaction.channelId);
  if (!channel?.isSendable()) {
    return;
  }

  const embed = buildTransactionEmbed(transaction);
  const components = buildTransactionButtons(transaction.id, transaction.statut);

  if (transaction.messageId) {
    try {
      const message = await channel.messages.fetch(transaction.messageId);
      await message.edit({ embeds: [embed], components });
      return;
    } catch {
      // message introuvable (supprimé) -> on en poste un nouveau ci-dessous
    }
  }

  const message = await channel.send({ embeds: [embed], components });
  await prisma.transaction.update({
    where: { id: transaction.id },
    data: { messageId: message.id },
  });
}

/**
 * Met à jour (ou crée) le message unique du tableau de bord listant toutes les
 * transactions de la guilde. Sans salon configuré, ne fait rien.
 */
export async function syncTransactionDashboard(client: Client, guildId: string) {
  const config = await getTransactionConfig(guildId);
  if (!config?.dashboardChannelId) {
    return;
  }

  const channel = await client.channels.fetch(config.dashboardChannelId).catch(() => null);
  if (!channel?.isSendable()) {
    logger.warn(`Salon de tableau de bord introuvable pour la guilde ${guildId}`);
    return;
  }

  const transactions = await prisma.transaction.findMany({
    where: { guildId },
    include: { items: true },
    orderBy: { updatedAt: "desc" },
  });

  const embeds = buildTransactionDashboardEmbeds(transactions);

  if (config.dashboardMessageId) {
    try {
      const message = await channel.messages.fetch(config.dashboardMessageId);
      await message.edit({ embeds });
      return;
    } catch {
      // message supprimé -> on en repost un nouveau ci-dessous
    }
  }

  const message = await channel.send({ embeds });
  await upsertTransactionConfig({ guildId, dashboardMessageId: message.id });
}

/** Rafraîchit l'embed de la transaction et le tableau de bord de la guilde. */
export async function refreshTransaction(client: Client, transactionId: string, guildId: string) {
  await syncTransactionMessage(client, transactionId);
  await syncTransactionDashboard(client, guildId);
}
