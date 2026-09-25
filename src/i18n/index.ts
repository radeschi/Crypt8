import { en } from "./locales/en.ts";
import { de } from "./locales/de.ts";
import { es } from "./locales/es.ts";
import { fr } from "./locales/fr.ts";
import { ja } from "./locales/ja.ts";
import { ptBR } from "./locales/pt-BR.ts";
import { zhCN } from "./locales/zh-CN.ts";
import type { LocaleId, Messages } from "./types.ts";

export type { LocaleId, Messages };

const catalogs: Record<LocaleId, Messages> = {
  "pt-BR": ptBR,
  en,
  es,
  de,
  fr,
  ja,
  "zh-CN": zhCN,
};

export function resolveLocale(tag: string | undefined | null): LocaleId {
  const value = (tag ?? "").split("_").join("-").toLowerCase();
  if (value.startsWith("pt")) return "pt-BR";
  if (value.startsWith("zh")) return "zh-CN";
  if (value.startsWith("en")) return "en";
  if (value.startsWith("es")) return "es";
  if (value.startsWith("de")) return "de";
  if (value.startsWith("fr")) return "fr";
  if (value.startsWith("ja")) return "ja";
  return "en";
}

export function detectLocale(): LocaleId {
  const meta = import.meta as { env?: { VITE_LOCALE?: string } };
  const override = meta.env?.VITE_LOCALE;
  if (override) return resolveLocale(override);
  if (typeof navigator === "undefined") return "en";
  return resolveLocale(navigator.language);
}

export function messagesFor(locale: LocaleId): Messages {
  return catalogs[locale];
}

export const locale = detectLocale();
export const messages = messagesFor(locale);

if (typeof document !== "undefined") {
  document.documentElement.lang = locale;
}
