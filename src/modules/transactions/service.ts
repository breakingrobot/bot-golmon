import type { Client, TextChannel } from "discord.js";
import { TransactionStatus, TransactionType } from "@prisma/client";
import { prisma } from "../../lib/prisma.js";
import { buildTransactionEmbed } from "../../lib/embeds/transactionEmbed.js";

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

export async function getTransaction(transactionId: string) {
  return prisma.transaction.findUnique({
    where: { id: transactionId },
    include: { items: true },
  });
}

export async function findOpenTransactions(guildId: string, query: string) {
  return prisma.transaction.findMany({
    where: {
      guildId,
      statut: { notIn: [TransactionStatus.LIVRE_PAYE, TransactionStatus.ANNULEE] },
      groupe: { contains: query },
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
  if (!channel?.isTextBased()) {
    return;
  }

  const embed = buildTransactionEmbed(transaction);

  if (transaction.messageId) {
    try {
      const message = await channel.messages.fetch(transaction.messageId);
      await message.edit({ embeds: [embed] });
      return;
    } catch {
      // message introuvable (supprimé) -> on en poste un nouveau ci-dessous
    }
  }

  const message = await (channel as TextChannel).send({ embeds: [embed] });
  await prisma.transaction.update({
    where: { id: transaction.id },
    data: { messageId: message.id },
  });
}
