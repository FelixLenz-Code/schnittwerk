// Interface language: German (source texts) or English.
//
// Texts are written in German and passed through `t()`; the German text is
// the key into the English dictionary (`en.ts`). Missing entries show the
// German text. Placeholders: t("{n} Objekte", { n: 3 }).

import { EN, EN_PATTERNS } from "./en";

export type Lang = "de" | "en";

const KEY = "schnittwerk.lang";

function initialLang(): Lang {
  try {
    const saved = localStorage.getItem(KEY);
    if (saved === "de" || saved === "en") return saved;
  } catch {}
  return (navigator.language || "de").toLowerCase().startsWith("de") ? "de" : "en";
}

export const i18n = $state<{ lang: Lang }>({ lang: initialLang() });

export function setLang(lang: Lang) {
  i18n.lang = lang;
  document.documentElement.lang = lang;
  try {
    localStorage.setItem(KEY, lang);
  } catch {}
}

/** Locale for numbers and dates. */
export function locale(): string {
  return i18n.lang === "de" ? "de-DE" : "en-GB";
}

function fill(text: string, params?: Record<string, string | number>): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (m, k) => (k in params ? String(params[k]) : m));
}

/** Translates a German UI text. */
export function t(de: string, params?: Record<string, string | number>): string {
  const text = i18n.lang === "en" ? (EN[de] ?? de) : de;
  return fill(text, params);
}

/**
 * Translates a message that may come from the backend (errors): exact texts
 * first, then patterns with a variable part.
 */
export function tMsg(msg: string): string {
  if (i18n.lang === "de") return msg;
  if (EN[msg]) return EN[msg];
  for (const [re, en] of EN_PATTERNS) {
    const m = msg.match(re);
    if (m) return en.replace(/\$(\d)/g, (_, i) => tMsg(m[Number(i)] ?? ""));
  }
  return msg;
}

if (typeof document !== "undefined") document.documentElement.lang = i18n.lang;
