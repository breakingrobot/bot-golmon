import { ActionRowBuilder, StringSelectMenuBuilder } from "discord.js";
import { encodeCustomId } from "../customId.js";
import {
  PROSPECT_NAMESPACE,
  SELECT_MAX_OPTIONS,
  VOTE_EMOJIS,
  VOTE_LABELS,
  VOTE_ORDER,
} from "../../modules/prospects/constants.js";
import type { SessionWithEntries } from "../../modules/prospects/service.js";

type Entry = SessionWithEntries["entries"][number];
type Row = ActionRowBuilder<StringSelectMenuBuilder>;

/**
 * Les libellés d'un select menu sont du texte brut : une mention `<@id>` n'y
 * serait pas rendue. L'appelant fournit donc les pseudos résolus depuis le
 * cache des membres, avec repli sur l'identifiant si le membre a quitté.
 */
export type ProspectLabels = Map<string, string>;

function optionFor(entry: Entry, labels: ProspectLabels) {
  const name = labels.get(entry.userId) ?? entry.userId;
  return {
    label: name.slice(0, 100),
    value: entry.id,
    description: entry.vetoById ? "⛔ Sous véto" : undefined,
  };
}

function selectRow(
  action: string,
  sessionId: string,
  placeholder: string,
  entries: Entry[],
  labels: ProspectLabels,
): Row {
  return new ActionRowBuilder<StringSelectMenuBuilder>().addComponents(
    new StringSelectMenuBuilder()
      .setCustomId(encodeCustomId(PROSPECT_NAMESPACE, action, sessionId))
      .setPlaceholder(placeholder)
      .addOptions(entries.map((entry) => optionFor(entry, labels))),
  );
}

/**
 * Trois menus : voter, poser un véto, consulter le détail. Sans prospect,
 * aucun composant — un select menu vide est refusé par l'API.
 *
 * Au-delà de 25 prospects, les menus sont tronqués (limite Discord) ; les
 * commandes `/prospect avis` et `/prospect veto` restent utilisables pour les
 * autres.
 */
export function buildProspectComponents(session: SessionWithEntries, labels: ProspectLabels): Row[] {
  const entries = session.entries.slice(0, SELECT_MAX_OPTIONS);
  if (entries.length === 0) {
    return [];
  }

  return [
    selectRow("vote", session.id, "🗳️ Donner mon avis sur…", entries, labels),
    selectRow("veto", session.id, "⛔ Poser un véto (leads)", entries, labels),
    selectRow("detail", session.id, "🔍 Voir les avis détaillés", entries, labels),
  ];
}

/** Options du choix de vote, partagées entre la modal et la commande. */
export const VOTE_SELECT_OPTIONS = VOTE_ORDER.map((choix) => ({
  label: VOTE_LABELS[choix],
  value: choix,
  emoji: VOTE_EMOJIS[choix],
}));
