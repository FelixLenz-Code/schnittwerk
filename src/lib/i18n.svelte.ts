// Interface language: German (source texts), English, French, Spanish, Dutch.
//
// Texts are written in German and passed through `t()`; the German text is
// the key into the dictionaries (`en.ts`, `fr.ts`, `es.ts`, `nl.ts`). Missing
// entries fall back to English, then to the German text.
// Placeholders: t("{n} Objekte", { n: 3 }).

import { EN, EN_PATTERNS } from "./en";
import { FR, FR_PATTERNS } from "./fr";
import { ES, ES_PATTERNS } from "./es";
import { NL, NL_PATTERNS } from "./nl";

export type Lang = "de" | "en" | "fr" | "es" | "nl";

/** Languages in the order shown in the settings, with their own names. */
export const LANGS: { id: Lang; name: string }[] = [
  { id: "de", name: "Deutsch" },
  { id: "en", name: "English" },
  { id: "fr", name: "Français" },
  { id: "es", name: "Español" },
  { id: "nl", name: "Nederlands" },
];

type Patterns = [RegExp, string][];
const DICTS: Record<Exclude<Lang, "de">, { words: Record<string, string>; patterns: Patterns }> = {
  en: { words: EN, patterns: EN_PATTERNS },
  fr: { words: FR, patterns: FR_PATTERNS },
  es: { words: ES, patterns: ES_PATTERNS },
  nl: { words: NL, patterns: NL_PATTERNS },
};

const LOCALES: Record<Lang, string> = { de: "de-DE", en: "en-GB", fr: "fr-FR", es: "es-ES", nl: "nl-NL" };

const KEY = "schnittwerk.lang";

function isLang(v: unknown): v is Lang {
  return typeof v === "string" && LANGS.some((l) => l.id === v);
}

function initialLang(): Lang {
  try {
    const saved = localStorage.getItem(KEY);
    if (isLang(saved)) return saved;
  } catch {}
  const sys = (navigator.language || "de").toLowerCase().slice(0, 2);
  return isLang(sys) ? sys : "en";
}

export const i18n = $state<{ lang: Lang }>({ lang: initialLang() });

export function setLang(lang: Lang) {
  if (!isLang(lang)) return;
  i18n.lang = lang;
  document.documentElement.lang = lang;
  try {
    localStorage.setItem(KEY, lang);
  } catch {}
}

/** Locale for numbers and dates. */
export function locale(): string {
  return LOCALES[i18n.lang];
}

/** True if the language writes decimals with a comma. */
export function decimalComma(): boolean {
  return i18n.lang !== "en";
}

function fill(text: string, params?: Record<string, string | number>): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (m, k) => (k in params ? String(params[k]) : m));
}

function lookup(de: string): string | undefined {
  if (i18n.lang === "de") return de;
  return DICTS[i18n.lang].words[de] ?? EN[de];
}

/** Translates a German UI text. */
export function t(de: string, params?: Record<string, string | number>): string {
  return fill(lookup(de) ?? de, params);
}

/**
 * Translates a message that may come from the backend (errors): exact texts
 * first, then patterns with a variable part.
 */
export function tMsg(msg: string): string {
  if (i18n.lang === "de") return msg;
  const exact = lookup(msg);
  if (exact) return exact;
  const own = DICTS[i18n.lang].patterns;
  for (const patterns of own === EN_PATTERNS ? [own] : [own, EN_PATTERNS]) {
    for (const [re, tr] of patterns) {
      const m = msg.match(re);
      if (m) return tr.replace(/\$(\d)/g, (_, i) => tMsg(m[Number(i)] ?? ""));
    }
  }
  return msg;
}

if (typeof document !== "undefined") document.documentElement.lang = i18n.lang;
