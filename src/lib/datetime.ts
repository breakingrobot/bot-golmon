import { addDays, format } from "date-fns";
import { fromZonedTime, toZonedTime } from "date-fns-tz";

/**
 * Le conteneur de prod tourne en UTC : sans conversion explicite, une saisie
 * "21:00" serait enregistrée à 21:00 UTC (23:00 heure française). Toutes les
 * saisies sont donc interprétées dans ce fuseau, DST comprise.
 */
export const TIMEZONE = "Europe/Paris";

const DATE_PATTERN = /^(\d{1,2})\/(\d{1,2})\/(\d{4})$/;
const TIME_PATTERN = /^(\d{1,2})[:hH](\d{2})$/;

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

/** Date du jour en heure de Paris, au format JJ/MM/AAAA. */
export function todayLabel(): string {
  return format(toZonedTime(new Date(), TIMEZONE), "dd/MM/yyyy");
}

/**
 * Convertit une saisie JJ/MM/AAAA + HH:MM (heure de Paris) en Date UTC.
 * Renvoie null si le format ou la date sont invalides (ex: 31/02).
 */
export function parseEventDate(dateLabel: string, timeLabel: string): Date | null {
  const dateMatch = DATE_PATTERN.exec(dateLabel.trim());
  const timeMatch = TIME_PATTERN.exec(timeLabel.trim());
  if (!dateMatch || !timeMatch) {
    return null;
  }

  const day = Number(dateMatch[1]);
  const month = Number(dateMatch[2]);
  const year = Number(dateMatch[3]);
  const hours = Number(timeMatch[1]);
  const minutes = Number(timeMatch[2]);

  if (month < 1 || month > 12 || day < 1 || day > 31 || hours > 23 || minutes > 59) {
    return null;
  }

  const parsed = fromZonedTime(
    `${year}-${pad(month)}-${pad(day)}T${pad(hours)}:${pad(minutes)}:00`,
    TIMEZONE,
  );
  if (Number.isNaN(parsed.getTime())) {
    return null;
  }

  // Attrape les dates qui débordent (31/02 -> 03/03) en revalidant le jour.
  const roundTrip = toZonedTime(parsed, TIMEZONE);
  if (roundTrip.getDate() !== day || roundTrip.getMonth() + 1 !== month) {
    return null;
  }

  return parsed;
}

/**
 * Résout la date d'un événement. Sans date explicite on prend aujourd'hui, et
 * si l'heure est déjà passée on bascule sur demain.
 */
export function resolveEventDate(
  dateLabel: string | null,
  timeLabel: string,
): { date: Date; rolledToTomorrow: boolean } | null {
  if (dateLabel) {
    const explicit = parseEventDate(dateLabel, timeLabel);
    return explicit ? { date: explicit, rolledToTomorrow: false } : null;
  }

  const today = parseEventDate(todayLabel(), timeLabel);
  if (!today) {
    return null;
  }
  if (today.getTime() > Date.now()) {
    return { date: today, rolledToTomorrow: false };
  }

  const tomorrowLabel = format(addDays(toZonedTime(new Date(), TIMEZONE), 1), "dd/MM/yyyy");
  const tomorrow = parseEventDate(tomorrowLabel, timeLabel);
  return tomorrow ? { date: tomorrow, rolledToTomorrow: true } : null;
}
