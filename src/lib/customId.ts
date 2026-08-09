const SEPARATOR = ":";

export interface ParsedCustomId {
  namespace: string;
  action: string;
  id: string;
  payload?: string;
}

export function encodeCustomId(
  namespace: string,
  action: string,
  id: string,
  payload?: string,
): string {
  const parts = [namespace, action, id];
  if (payload) {
    parts.push(payload);
  }
  return parts.join(SEPARATOR);
}

export function decodeCustomId(customId: string): ParsedCustomId | null {
  const [namespace, action, id, payload] = customId.split(SEPARATOR);
  if (!namespace || !action || !id) {
    return null;
  }
  return { namespace, action, id, payload };
}
