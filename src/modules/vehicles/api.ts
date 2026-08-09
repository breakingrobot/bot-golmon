import { logger } from "../../lib/logger.js";

const API_URL = "https://api.glife.fr/roleplay/vehicles";
const TIMEOUT_MS = 8000;
const CACHE_TTL_MS = 5 * 60 * 1000;

export interface ApiVehicleParty {
  id: string;
  name: string;
  type?: number;
}

export interface ApiVehicle {
  id: string;
  plate: string;
  /** Nom lisible, ex: "Kamacho". */
  name: string | null;
  /** Code interne du modèle, ex: "kamacho". */
  model: string | null;
  illegal: boolean;
  /** Propriétaire en jeu : joueur ou organisation, distinct du propriétaire Discord. */
  owner: ApiVehicleParty | null;
  buyer: ApiVehicleParty | null;
}

export type LookupResult =
  | { status: "found"; vehicle: ApiVehicle }
  | { status: "not_found" }
  | { status: "error"; reason: string };

interface CacheEntry {
  result: LookupResult;
  expiresAt: number;
}

const cache = new Map<string, CacheEntry>();
const inFlight = new Map<string, Promise<LookupResult>>();

/** Majuscules, sans espaces ni tirets : évite les doublons et les recherches ratées. */
export function normalizePlate(plate: string): string {
  return plate.trim().toUpperCase().replaceAll(/[\s-]/g, "");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function optionalString(value: unknown): string | null {
  if (typeof value === "string") {
    return value;
  }
  return typeof value === "number" ? String(value) : null;
}

/** Les identifiants ne sont pas typés uniformément par l'API (owner.id string, buyer.id number). */
function parseParty(value: unknown): ApiVehicleParty | null {
  if (!isRecord(value)) {
    return null;
  }
  const name = optionalString(value.name);
  if (!name) {
    return null;
  }
  return {
    id: optionalString(value.id) ?? "",
    name,
    type: typeof value.type === "number" ? value.type : undefined,
  };
}

function parseVehicle(value: unknown): ApiVehicle | null {
  if (!isRecord(value)) {
    return null;
  }
  const plate = optionalString(value.plate);
  if (!plate) {
    return null;
  }
  return {
    id: optionalString(value.id) ?? "",
    plate,
    name: optionalString(value.name),
    model: optionalString(value.model),
    // `illegal` est un entier 0/1 côté API, pas un booléen.
    illegal: Boolean(value.illegal),
    owner: parseParty(value.owner),
    buyer: parseParty(value.buyer),
  };
}

async function requestPlate(plate: string): Promise<LookupResult> {
  try {
    const response = await fetch(`${API_URL}?plate=${encodeURIComponent(plate)}`, {
      headers: { accept: "application/json" },
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });

    // 404 = plaque inconnue, ce n'est pas une panne.
    if (response.status === 404) {
      return { status: "not_found" };
    }
    if (!response.ok) {
      return { status: "error", reason: `HTTP ${response.status}` };
    }

    const payload: unknown = await response.json();
    // L'API renvoie un tableau : un tableau vide vaut "non trouvé".
    const first = Array.isArray(payload) ? payload[0] : payload;
    const vehicle = parseVehicle(first);

    return vehicle ? { status: "found", vehicle } : { status: "not_found" };
  } catch (error) {
    const reason = error instanceof Error ? error.message : "erreur inconnue";
    logger.warn(`Consultation API échouée pour la plaque ${plate}: ${reason}`);
    return { status: "error", reason };
  }
}

/**
 * Consulte l'API pour une plaque. Les résultats sont mis en cache 5 min et les
 * appels concurrents sur la même plaque partagent une seule requête.
 */
export async function lookupPlate(rawPlate: string): Promise<LookupResult> {
  const plate = normalizePlate(rawPlate);
  if (!plate) {
    return { status: "error", reason: "plaque vide" };
  }

  const cached = cache.get(plate);
  if (cached && cached.expiresAt > Date.now()) {
    return cached.result;
  }

  const pending = inFlight.get(plate);
  if (pending) {
    return pending;
  }

  const request = requestPlate(plate)
    .then((result) => {
      // Les erreurs ne sont pas mises en cache : la prochaine tentative doit réessayer.
      if (result.status !== "error") {
        cache.set(plate, { result, expiresAt: Date.now() + CACHE_TTL_MS });
      }
      return result;
    })
    .finally(() => {
      inFlight.delete(plate);
    });

  inFlight.set(plate, request);
  return request;
}
