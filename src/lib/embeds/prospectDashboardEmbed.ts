import { EmbedBuilder } from "discord.js";
import type { ProspectVote, ProspectVoteChoice } from "@prisma/client";
import { mention, packFields, truncateField, type EmbedField } from "../format.js";
import { formatDateLabel } from "../datetime.js";
import {
  MOTIF_PREVIEW,
  PROSPECT_COLOR,
  VETO_COLOR,
  VOTE_EMOJIS,
  VOTE_ORDER,
} from "../../modules/prospects/constants.js";
import type { SessionWithEntries } from "../../modules/prospects/service.js";

/** Titre, pied de page et marges de l'API : on ne remplit pas les 6000 caractères. */
const FIELD_BUDGET = 5200;
/** Discord plafonne à 25 champs : on en garde un pour le résumé « … ». */
const MAX_PROSPECT_FIELDS = 24;

type Entry = SessionWithEntries["entries"][number];

/** `✅ 3 · ❌ 1 · ⬜ 0` — toujours les trois choix, même à zéro. */
export function formatTally(votes: ProspectVote[]): string {
  const counts = votes.reduce<Partial<Record<ProspectVoteChoice, number>>>((acc, vote) => {
    acc[vote.choix] = (acc[vote.choix] ?? 0) + 1;
    return acc;
  }, {});

  return VOTE_ORDER.map((choix) => `${VOTE_EMOJIS[choix]} ${counts[choix] ?? 0}`).join(" · ");
}

function formatVoteLine(vote: ProspectVote, limit = MOTIF_PREVIEW): string {
  const motif = vote.motif.length > limit ? `${vote.motif.slice(0, limit - 1)}…` : vote.motif;
  return `${VOTE_EMOJIS[vote.choix]} ${mention(vote.voterId)} — ${motif}`;
}

/** Détail complet d'un prospect, pour les réponses éphémères et l'historique. */
export function formatEntryDetail(entry: Entry): string {
  const veto = entry.vetoById
    ? `⛔ **Véto** de ${mention(entry.vetoById)} — ${entry.vetoMotif ?? "sans motif"}\n\n`
    : "";
  const votes =
    entry.votes.length === 0
      ? "*Aucun avis pour le moment.*"
      : entry.votes.map((vote) => formatVoteLine(vote, 400)).join("\n");
  return truncateField(`${veto}${votes}`);
}

export function buildProspectDashboardEmbed(session: SessionWithEntries): EmbedBuilder {
  const hasVeto = session.entries.some((entry) => entry.vetoById);

  const embed = new EmbedBuilder()
    .setTitle(`🗳️ Avis prospects — semaine du ${formatDateLabel(session.weekStart).slice(0, 10)}`)
    .setColor(hasVeto ? VETO_COLOR : PROSPECT_COLOR)
    .setTimestamp();

  if (session.entries.length === 0) {
    embed.setDescription(
      "Aucun prospect cette semaine. Vérifie le rôle prospect avec `/prospect config`, ou ajoute quelqu'un via `/prospect ajouter`.",
    );
    return embed;
  }

  // Prospects sous véto en tête : c'est l'information la plus décisive, et
  // `packFields` tronque par la fin si l'embed déborde.
  const entries = session.entries.toSorted(
    (a, b) => Number(Boolean(b.vetoById)) - Number(Boolean(a.vetoById)),
  );

  const fields: EmbedField[] = entries.map((entry) => ({
    name: `${entry.vetoById ? "⛔ " : "👤 "}${formatTally(entry.votes)}`,
    value: truncateField(
      [
        mention(entry.userId),
        entry.vetoById
          ? `⛔ **Véto** de ${mention(entry.vetoById)} — ${entry.vetoMotif ?? "sans motif"}`
          : null,
        ...entry.votes.map((vote) => formatVoteLine(vote)),
        entry.votes.length === 0 ? "*Aucun avis pour le moment.*" : null,
      ]
        .filter((line): line is string => line !== null)
        .join("\n"),
    ),
  }));

  const { kept, dropped } = packFields(fields, {
    budget: FIELD_BUDGET,
    maxFields: MAX_PROSPECT_FIELDS,
  });
  embed.addFields(kept);

  if (dropped.length > 0) {
    embed.addFields({
      name: "…",
      value: `+${dropped.length} prospect(s) non affiché(s) — utilise \`/prospect avis\` pour les consulter.`,
    });
  }

  const totalVotes = session.entries.reduce((total, entry) => total + entry.votes.length, 0);
  embed.setFooter({
    text: `${session.entries.length} prospect(s) · ${totalVotes} avis · un avis par gradé, modifiable`,
  });

  return embed;
}
