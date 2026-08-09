import { ActionRowBuilder, ButtonBuilder, ButtonStyle } from "discord.js";
import { TransactionStatus } from "@prisma/client";
import { encodeCustomId } from "../customId.js";

const TRANSITIONS: { status: TransactionStatus; label: string; style: ButtonStyle }[] = [
  { status: TransactionStatus.EN_ATTENTE_RDV, label: "En attente RDV", style: ButtonStyle.Primary },
  { status: TransactionStatus.AVANCE_ATTENTE_PAIEMENT, label: "Avance", style: ButtonStyle.Primary },
  { status: TransactionStatus.LIVRE_PAYE, label: "Livré et payé", style: ButtonStyle.Success },
  { status: TransactionStatus.ANNULEE, label: "Annuler", style: ButtonStyle.Danger },
];

export function buildTransactionButtons(
  transactionId: string,
  currentStatus: TransactionStatus,
): ActionRowBuilder<ButtonBuilder>[] {
  const statusRow = new ActionRowBuilder<ButtonBuilder>().addComponents(
    ...TRANSITIONS.map((transition) =>
      new ButtonBuilder()
        .setCustomId(encodeCustomId("transaction", "status", transactionId, transition.status))
        .setLabel(transition.label)
        .setStyle(transition.style)
        // On désactive le statut courant : le clic serait un no-op.
        .setDisabled(transition.status === currentStatus),
    )
  );

  const actionsRow = new ActionRowBuilder<ButtonBuilder>().addComponents(
    new ButtonBuilder()
      .setCustomId(encodeCustomId("transaction", "additem", transactionId))
      .setLabel("Ajouter un objet")
      .setStyle(ButtonStyle.Secondary)
      .setEmoji("➕"),
    new ButtonBuilder()
      .setCustomId(encodeCustomId("transaction", "delete", transactionId))
      .setLabel("Supprimer")
      .setStyle(ButtonStyle.Danger)
      .setEmoji("🗑️")
  );

  return [statusRow, actionsRow];
}
