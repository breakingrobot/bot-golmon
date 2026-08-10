import { ProspectVoteChoice } from "@prisma/client";

/** Namespace des `custom_id` du module (cf. `src/lib/customId.ts`). */
export const PROSPECT_NAMESPACE = "prospect";

export const VOTE_LABELS: Record<ProspectVoteChoice, string> = {
  [ProspectVoteChoice.POUR]: "Pour",
  [ProspectVoteChoice.CONTRE]: "Contre",
  [ProspectVoteChoice.BLANC]: "Blanc",
};

export const VOTE_EMOJIS: Record<ProspectVoteChoice, string> = {
  [ProspectVoteChoice.POUR]: "✅",
  [ProspectVoteChoice.CONTRE]: "❌",
  [ProspectVoteChoice.BLANC]: "⬜",
};

/** Ordre d'affichage stable, indépendant de l'ordre de déclaration de l'enum. */
export const VOTE_ORDER: ProspectVoteChoice[] = [
  ProspectVoteChoice.POUR,
  ProspectVoteChoice.CONTRE,
  ProspectVoteChoice.BLANC,
];

export const VOTE_CHOICES = VOTE_ORDER.map((choix) => ({
  name: `${VOTE_EMOJIS[choix]} ${VOTE_LABELS[choix]}`,
  value: choix,
}));

export const PROSPECT_COLOR = 0x5865f2;
export const VETO_COLOR = 0xed4245;

/** Longueur des motifs dans le tableau de bord ; le détail complet est ailleurs. */
export const MOTIF_PREVIEW = 90;

export const MOTIF_MAX_LENGTH = 500;

/** Discord plafonne un select menu à 25 options. */
export const SELECT_MAX_OPTIONS = 25;

export function isVoteChoice(value: string): value is ProspectVoteChoice {
  return (Object.values(ProspectVoteChoice) as string[]).includes(value);
}
