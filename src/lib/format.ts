/**
 * Limites Discord : 1024 caractères par champ d'embed, 2000 par message.
 * Une mention fait ~22 caractères, donc un rôle d'une cinquantaine de membres
 * suffit à dépasser la limite d'un champ et à faire échouer l'edit
 * (`Invalid Form Body`). Ces helpers bornent systématiquement les listes.
 */
const FIELD_LIMIT = 1024;
const MESSAGE_LIMIT = 2000;

export function mention(userId: string): string {
  return `<@${userId}>`;
}

/**
 * Liste de mentions pour un champ d'embed, tronquée à `max` entrées puis à la
 * limite de caractères. Renvoie "Aucun" si la liste est vide.
 */
export function formatMentionList(
  userIds: string[],
  options: { max?: number; detailByUserId?: Map<string, string>; empty?: string } = {},
): string {
  const { max = Infinity, detailByUserId, empty = "Aucun" } = options;

  if (userIds.length === 0) {
    return empty;
  }

  const shown = userIds.slice(0, max);
  const lines = shown.map((id) => {
    const detail = detailByUserId?.get(id);
    return detail ? `${mention(id)} (${detail})` : mention(id);
  });

  const validLines = lines.reduce(
    (acc, line) => {
      if (acc.text.length + line.length + 1 <= FIELD_LIMIT - 20) {
        acc.lines.push(line);
        acc.text += (acc.text ? "\n" : "") + line;
      }
      return acc;
    },
    { lines: [] as string[], text: "" },
  );

  const remaining = userIds.length - validLines.lines.length;
  return remaining > 0 ? `${validLines.text}\n… +${remaining} autre${remaining > 1 ? "s" : ""}` : validLines.text;
}

/**
 * Découpe une liste de mentions en plusieurs messages sous la limite de 2000
 * caractères, chacun préfixé par `prefix`.
 */
export function chunkMentions(userIds: string[], prefix: string): string[] {
  const result = userIds.reduce(
    (acc, userId) => {
      const candidate = acc.current ? `${acc.current} ${mention(userId)}` : `${prefix} ${mention(userId)}`;
      if (candidate.length > MESSAGE_LIMIT) {
        acc.chunks.push(acc.current);
        acc.current = `${prefix} ${mention(userId)}`;
      } else {
        acc.current = candidate;
      }
      return acc;
    },
    { chunks: [] as string[], current: "" },
  );

  if (result.current) {
    result.chunks.push(result.current);
  }

  return result.chunks;
}

/** Divise une liste en morceaux de taille maximale `chunkSize`. */
export function chunkMentionsList(userIds: string[], chunkSize: number): string[][] {
  return Array.from({ length: Math.ceil(userIds.length / chunkSize) }, (_, i) =>
    userIds.slice(i * chunkSize, (i + 1) * chunkSize),
  );
}

/** Tronque un texte libre pour un champ d'embed. */
export function truncateField(text: string, limit = FIELD_LIMIT): string {
  return text.length <= limit ? text : `${text.slice(0, limit - 1)}…`;
}
