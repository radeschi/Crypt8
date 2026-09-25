import type { Messages } from "../i18n/types.ts";
import type { ProgressStatus } from "../types/progress.ts";

export function formatBytes(bytes: number, locale = "pt-BR"): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  const digits = index === 0 || value >= 100 ? 0 : 1;
  return `${value.toLocaleString(locale, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  })} ${units[index]}`;
}

export function formatRate(bytesPerSecond: number, locale = "pt-BR"): string {
  if (!Number.isFinite(bytesPerSecond) || bytesPerSecond <= 0) {
    return "";
  }
  return `${formatBytes(bytesPerSecond, locale)}/s`;
}

export function formatRemaining(
  seconds: number | null,
  status: ProgressStatus,
  copy: Pick<Messages, "calculating" | "finalizing" | "seconds" | "minutes" | "hours">,
): string {
  if (status === "finalizing" || (seconds != null && seconds < 1)) {
    return copy.finalizing;
  }
  if (seconds == null || status === "preparing") {
    return copy.calculating;
  }
  const rounded = Math.max(1, Math.round(seconds));
  if (rounded < 60) {
    return copy.seconds(rounded);
  }
  const minutes = Math.max(1, Math.round(rounded / 60));
  if (minutes < 60) {
    return copy.minutes(minutes);
  }
  return copy.hours(Math.max(1, Math.round(minutes / 60)));
}
