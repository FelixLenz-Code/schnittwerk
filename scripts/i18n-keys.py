#!/usr/bin/env python3
"""Lists the German UI texts of the frontend and checks the dictionaries
(src/lib/en.ts, fr.ts, es.ts, nl.ts).

  python3 scripts/i18n-keys.py          # missing entries per language
  python3 scripts/i18n-keys.py --all    # all keys
"""
import glob, json, re, sys

STR = r'"((?:[^"\\\n]|\\.)*)"'
PATTERNS = [
    r'\bt\(\s*' + STR,                       # t("…")
    r'\btMsg\(\s*(?:[\w.?]+\s*\?\?\s*)?' + STR,  # tMsg(x ?? "…")
    r'\b(?:label|hint)=' + STR,              # <Slider label="…" hint="…">
    r'\b(?:label|hint|name):\s*' + STR,      # { label: "…" }
    r'editor\.(?:toast|error)\(\s*' + STR,   # editor.toast("…")
    r'new Error\(\s*' + STR,
    r'\?\s*' + STR + r'\s*:\s*' + STR,       # cond ? "…" : "…" inside toasts
]
GERMAN = re.compile(r'[äöüßÄÖÜ]|\b(der|die|das|und|nicht|mit|für|ein|eine|ist|wird|auf|zum|beim|nach|von|im|aus|bei|oder|zu|den|dem)\b|[A-ZÄÖÜ][a-zäöüß]{2,}')

def keys():
    out = set()
    for f in glob.glob('src/**/*.svelte', recursive=True) + glob.glob('src/**/*.ts', recursive=True):
        if f.endswith(tuple(f'/{l}.ts' for l in LANGS) + ('i18n.svelte.ts',)):
            continue
        s = open(f, encoding='utf-8').read()
        for p in PATTERNS:
            for m in re.finditer(p, s):
                for g in m.groups():
                    if g and GERMAN.search(g) and not g.startswith(('#', 'M0', 'schnittwerk.')):
                        out.add(json.loads('"' + g + '"'))
        # values of string maps like STATE / TIPS: `key: "…",`
        for m in re.finditer(r'^\s+\w+:\s*' + STR + r',?\s*$', s, re.M):
            g = m.group(1)
            if GERMAN.search(g) and ' ' in g or re.fullmatch(r'[A-ZÄÖÜa-zäöüß…\- ]+', g or ''):
                out.add(json.loads('"' + g + '"'))
    return out

LANGS = ['en', 'fr', 'es', 'nl']

def dictionary(lang):
    s = open(f'src/lib/{lang}.ts', encoding='utf-8').read().split('_PATTERNS')[0]
    out = set()
    for m in re.finditer(r'^\s*(?:' + STR + r'|([^\s:"/]+))\s*:', s, re.M):
        out.add(json.loads('"' + m.group(1) + '"') if m.group(1) is not None else m.group(2))
    return out

# Internal ids (icons, template ids …) that are not shown.
ID = re.compile(r'^[a-z]+(?:-[a-z]+)*$|^[a-z]+[A-Z]\w*$')

if __name__ == '__main__':
    k = {x for x in keys() if not ID.match(x)}
    if '--all' in sys.argv:
        print('\n'.join(sorted(k)))
    else:
        bad = False
        for lang in LANGS:
            missing = sorted(k - dictionary(lang))
            if missing:
                print(f'--- {lang}')
                print('\n'.join(missing))
            print(f'{lang}: {len(missing)} von {len(k)} ohne Übersetzung', file=sys.stderr)
            bad = bad or bool(missing)
        sys.exit(1 if bad else 0)
