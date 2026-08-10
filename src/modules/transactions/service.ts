import type { Client } from "discord.js";
import { TransactionStatus, type TransactionType } from "@prisma/client";
import { prisma } from "../../lib/prisma.js";
import { logger } from "../../lib/logger.js";
import { fetchChannelSafe } from "../../lib/discordClient.js";
import { buildTransactionEmbed } from "../../lib/embeds/transactionEmbed.js";
import { buildTransactionDashboardEmbed } from "../../lib/embeds/transactionDashboardEmbed.js";
import { buildTransactionButtons } from "../../lib/components/transactionButtons.js";
import { ACTIVE_STATUSES, TERMINAL_STATUSES } from "./constants.js";

/** Nombre de transactions terminées listées par statut dans le tableau de bord. */
const TERMINAL_DASHBOARD_LIMIT = 10;

export async function getTransactionConfig(guildId: string) {
  return prisma.transactionConfig.findUnique({ where: { guildId } });
}

export async function upsertTransactionConfig(params: {
  guildId: string;
  dashboardChannelId?: string;
  dashboardMessageId?: string | null;
  pingRoleId?: string | null;
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

export async function setTransactionStatus(
  guildId: string,
  transactionId: string,
  statut: TransactionStatus,
) {
  const { count } = await prisma.transaction.updateMany({
    where: { id: transactionId, guildId },
    data: { statut },
  });
  return count > 0;
}

export async function deleteTransaction(guildId: string, transactionId: string) {
  const { count } = await prisma.transaction.deleteMany({
    where: { id: transactionId, guildId },
  });
  return count > 0;
}

/**
 * Toujours scoper à la guilde : l'identifiant d'une transaction est public (il
 * figure dans le pied de son embed) et l'autocomplétion Discord n'empêche pas
 * de saisir une valeur arbitraire. Sans ce filtre, n'importe qui pourrait lire
 * ou modifier la transaction d'un autre serveur.
 */
export async function getTransaction(guildId: string, transactionId: string) {
  return prisma.transaction.findFirst({
    where: { id: transactionId, guildId },
    include: { items: true },
  });
}

export async function findTransactions(guildId: string, query: string, openOnly = true) {
  return prisma.transaction.findMany({
    where: {
      guildId,
      groupe: { contains: query },
      ...(openOnly
        ? { statut: { notIn: [TransactionStatus.ANNULEE, TransactionStatus.CLOTUREE] } }
        : {}),
    },
    take: 25,
    orderBy: { createdAt: "desc" },
  });
}

export async function syncTransactionMessage(
  client: Client,
  guildId: string,
  transactionId: string,
) {
  const transaction = await getTransaction(guildId, transactionId);
  if (!transaction) {
    return;
  }

  const channel = await fetchChannelSafe(
    client,
    transaction.channelId,
    `le salon de la transaction ${transactionId}`,
  );
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

  // Le rôle n'est mentionné qu'à la toute première publication. Republier après
  // une suppression manuelle ne doit pas re-pinguer tout le rôle à chaque action.
  const isFirstPost = transaction.messageId === null;
  const config = isFirstPost ? await getTransactionConfig(transaction.guildId) : null;
  const content = config?.pingRoleId ? `<@&${config.pingRoleId}>` : undefined;

  const message = await channel.send({ content, embeds: [embed], components });
  await prisma.transaction.update({
    where: { id: transaction.id },
    data: { messageId: message.id },
  });
}

/**
 * Met à jour (ou crée) le message unique du tableau de bord listant les
 * transactions de la guilde. Sans salon configuré, ne fait rien.
 *
 * Les transactions en cours sont listées intégralement ; les terminées sont
 * bornées aux plus récentes, sans quoi le tableau de bord grossit indéfiniment
 * jusqu'à dépasser les limites d'un embed.
 */
export async function syncTransactionDashboard(client: Client, guildId: string) {
  const config = await getTransactionConfig(guildId);
  if (!config?.dashboardChannelId) {
    return;
  }

  const channel = await fetchChannelSafe(
    client,
    config.dashboardChannelId,
    `le salon de tableau de bord des transactions de la guilde ${guildId}`,
  );
  if (!channel?.isSendable()) {
    logger.warn(`Salon de tableau de bord introuvable pour la guilde ${guildId}`);
    return;
  }

  const [active, terminalBuckets, totalCount] = await Promise.all([
    prisma.transaction.findMany({
      where: { guildId, statut: { in: [...ACTIVE_STATUSES] } },
      include: { items: true },
      orderBy: { updatedAt: "desc" },
    }),
    Promise.all(
      TERMINAL_STATUSES.map((statut) =>
        prisma.transaction.findMany({
          where: { guildId, statut },
          include: { items: true },
          orderBy: { updatedAt: "desc" },
          take: TERMINAL_DASHBOARD_LIMIT,
        }),
      ),
    ),
    prisma.transaction.count({ where: { guildId } }),
  ]);

  const embed = buildTransactionDashboardEmbed([...active, ...terminalBuckets.flat()], totalCount);

  if (config.dashboardMessageId) {
    const message = await channel.messages.fetch(config.dashboardMessageId).catch(() => null);
    if (message) {
      await message.edit({ embeds: [embed] });
      return;
    }
    // Message supprimé : on en repost un et on mémorise le nouvel identifiant.
  }

  const message = await channel.send({ embeds: [embed] });
  await upsertTransactionConfig({ guildId, dashboardMessageId: message.id });
}

/** Rafraîchit l'embed de la transaction et le tableau de bord de la guilde. */
export async function refreshTransaction(client: Client, guildId: string, transactionId: string) {
  await syncTransactionMessage(client, guildId, transactionId);
  await syncTransactionDashboard(client, guildId);
}
