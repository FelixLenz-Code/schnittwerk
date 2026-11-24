#!/usr/bin/env python3
"""Collects the license texts of every library shipped in the .deb.

Rust: all crates reachable through normal dependencies from the shipped
binaries (app, CLI, MCP server) on x86_64 Linux. npm: the runtime packages
bundled into the frontend. Output: target/third-party-licenses.txt
"""
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "target" / "third-party-licenses.txt"
SHIPPED = {"schnittwerk", "schnittwerk-cli", "schnittwerk-mcp"}
LICENSE_PREFIXES = ("license", "licence", "copying", "notice", "unlicense", "copyright")

MIT = """Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""

BSD3 = """Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.
3. Neither the name of the copyright holder nor the names of its contributors
   may be used to endorse or promote products derived from this software
   without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE."""


def license_files(directory: pathlib.Path):
    files = []
    for f in sorted(directory.iterdir()):
        if f.is_file() and f.name.lower().startswith(LICENSE_PREFIXES):
            files.append(f)
    return files


def rust_packages():
    meta = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--locked",
         "--filter-platform", "x86_64-unknown-linux-gnu"], cwd=ROOT))
    pkgs = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    todo = [i for i, p in pkgs.items() if p["name"] in SHIPPED and p["source"] is None]
    seen = set()
    while todo:
        i = todo.pop()
        if i in seen:
            continue
        seen.add(i)
        for dep in nodes[i]["deps"]:
            if any(k["kind"] is None for k in dep["dep_kinds"]):
                todo.append(dep["pkg"])
    out = [pkgs[i] for i in seen if pkgs[i]["source"] is not None]
    return sorted(out, key=lambda p: (p["name"], p["version"]))


def npm_packages():
    pkg = json.loads((ROOT / "package.json").read_text())
    out = []
    for name in sorted(pkg.get("dependencies", {})):
        d = ROOT / "node_modules" / name
        meta = json.loads((d / "package.json").read_text())
        out.append((name, meta.get("version", "?"), meta.get("license", "?"), d))
    return out


def main():
    missing = []
    parts = [
        "Schnittwerk – Lizenzen der mitgelieferten Bibliotheken\n"
        "Schnittwerk selbst steht unter der GPL-3.0-or-later.\n"
        "Icons (Lucide/Feather): siehe THIRD_PARTY.md im Quellcode.\n",
    ]

    def section(title, license_expr, url, directory):
        texts = license_files(directory)
        body = [f"{'=' * 78}\n{title}\nLizenz: {license_expr}\n"]
        if url:
            body.append(f"Quelle: {url}\n")
        if not texts:
            missing.append(title)
            body.append("(Keine Lizenzdatei im Paket; es gilt der Standardtext der genannten Lizenz.)\n")
        for f in texts:
            body.append(f"\n--- {f.name} ---\n{f.read_text(errors='replace').strip()}\n")
        parts.append("".join(body))

    rust = rust_packages()
    for p in rust:
        section(f"{p['name']} {p['version']} (Rust)", p.get("license") or "?",
                p.get("repository") or "", pathlib.Path(p["manifest_path"]).parent)
    npm = npm_packages()
    for name, version, lic, d in npm:
        section(f"{name} {version} (npm)", lic, "", d)

    if missing:
        parts.append(f"{'=' * 78}\nStandardtexte der Lizenzen (für Pakete ohne eigene Lizenzdatei)\n\n"
                     f"--- MIT ---\n{MIT}\n\n--- BSD-3-Clause ---\n{BSD3}\n")
        for name, path in (("Apache-2.0", "Apache-2.0"), ("MPL-2.0", "MPL-2.0")):
            f = pathlib.Path("/usr/share/common-licenses") / path
            if f.exists():
                parts.append(f"--- {name} ---\n{f.read_text().strip()}\n")
            else:
                parts.append(f"--- {name} ---\nhttps://spdx.org/licenses/{name}.html\n")

    OUT.parent.mkdir(exist_ok=True)
    OUT.write_text("\n".join(parts))
    print(f"{OUT.relative_to(ROOT)}: {len(rust)} Rust-Crates, {len(npm)} npm-Pakete"
          f"{f', {len(missing)} ohne Lizenzdatei' if missing else ''}", file=sys.stderr)


if __name__ == "__main__":
    main()
