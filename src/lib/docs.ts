// Built-in help (help tab). Light markup: blank line = paragraph, "- " =
// list item, **bold**, `key`. Texts use backticks so the translation check
// (scripts/i18n-keys.py) does not treat them as UI strings.

export interface DocSection {
  id: string;
  heading: string;
  text: string;
}

export const DOCS_DE: DocSection[] = [
  {
    id: "start",
    heading: `Erste Schritte`,
    text: `Beim Start öffnet sich die **Startseite** (auch über einen Klick auf „Schnittwerk“ oben links). Dort wählst du, was du machen möchtest – Sticker, Vinyl, Flexfolie, Papier, Stift oder ein großes Motiv. Schnittwerk stellt dann Material, Matte und Seite passend ein.

- Bilder und Vektoren über **Importieren** (\`Strg+I\`) laden oder einfach auf die Arbeitsfläche ziehen.
- Rechts im Tab **Tracen** wird aus einem Bild eine Schnittlinie, im Tab **Sticker** eine Sticker-Kontur.
- Mit **Schneiden** (\`Strg+Enter\`) geht es zum Plotter.
- Die **Menüleiste** ganz oben enthält alle Funktionen – auch per Tastatur mit \`Alt\` und dem unterstrichenen Buchstaben. Unter **Datei → Einstellungen** stehen Sprache, Design, Raster und mehr.

Gespeichert wird als \`.schnitt\`-Datei, Bilder sind darin enthalten. Ungespeicherte Arbeit wird alle 20 Sekunden gesichert und nach einem Absturz auf der Startseite angeboten.`,
  },
  {
    id: "trace",
    heading: `Bilder tracen`,
    text: `Bild auswählen, Tab **Tracen** öffnen – die Vorschau aktualisiert sich bei jeder Einstellung.

- **S/W**: dunkle Bereiche werden zu Formen – für Zeichnungen, Schrift, Logos.
- **Farbe**: eine Farbe mit der Pipette aufnehmen.
- **Mehrfarbig**: eine Ebene pro Farbe.
- **Freistellen**: Umriss von PNGs mit Transparenz.
- **Linie**: eine Linie mitten durch jeden Strich – ideal für den Stift.

**Glättung** macht Kurven ruhiger, **Flecken entfernen** verwirft Krümel. Fotos von Zeichnungen lassen sich mit **Perspektive korrigieren** gerade ziehen und mit **Beleuchtung ausgleichen** von Schatten befreien. Mit **Radierer** und **Pinsel** bereinigst du das Bild direkt vor dem Tracen.`,
  },
  {
    id: "sticker",
    heading: `Sticker und Print & Cut`,
    text: `Für Sticker wird gedruckt und danach ausgeschnitten. Die Seite bekommt dafür **Passermarken**, die der Plotter vor dem Schnitt mit seinem Sensor sucht.

- **Print & Cut** oben einschalten (oder auf der Startseite „Sticker“ wählen).
- Bild auswählen, Tab **Sticker**: Hintergrund entfernen oder Transparenz nutzen, Rand einstellen, **Kontur übernehmen**.
- **Mehrere Bilder auf einmal**: alle auswählen – im Sticker-Tab erscheint **Sticker-Bogen erstellen**. Alle bekommen eine Kontur, auf Wunsch mehrere Exemplare, und werden platzsparend angeordnet.
- **Einzeln ausschneiden** schneidet zusätzlich mit Trägerrand durch (blau), sonst nur an (rot).
- Drucken (\`Strg+P\`) – im Druckdialog nie „An Seite anpassen“ wählen.
- Bogen oben links auf die Matte legen, laden, schneiden.

Liegt der Schnitt etwas daneben, im Schneiden-Dialog unter **Feinjustierung** mit dem Testbogen nachstellen.`,
  },
  {
    id: "cut",
    heading: `Schneiden`,
    text: `Im Schneiden-Dialog bekommt jede **Schnittfarbe** ein eigenes Material: rot = anschneiden, blau = durchschneiden, grün = zeichnen. Leichte Schnitte laufen zuerst, damit der Bogen zusammenhält.

- **Material** wählen; Werte bei Bedarf anpassen und als eigenes Material speichern.
- **Testschnitt**: ein kleines Quadrat an der angegebenen Stelle.
- **Testreihe**: mehrere Quadrate mit steigendem Druck oder steigender Messertiefe – danach das sauberste anklicken, der Wert wird übernommen.
- **Simulation**: zeigt vorher in Schnittreihenfolge, was der Plotter macht.
- Die **Dauer** wird geschätzt und mit jedem Schnitt genauer.
- Nach **Abbruch** oder Verbindungsfehler erscheint **Weiterschneiden** – der Schnitt geht dort weiter, wo er unterbrochen wurde. Matte dabei nicht bewegen.

Ist ein Schnitt fertig, während das Fenster im Hintergrund ist, meldet Schnittwerk das mit einer Benachrichtigung; der Ton lässt sich im Plotter-Menü abschalten.`,
  },
  {
    id: "vinyl",
    heading: `Vinyl und Flexfolie`,
    text: `**Vinyl**: Folie mit dem Trägerpapier nach unten auf die Matte. Nach dem Schneiden hilft die **Entgitterungshilfe** (Objekt-Tab): ein Rahmen und Linien durch den Abfall, damit sich die Folie in Streifen abziehen lässt.

**Flexfolie (Textil)**: glänzende Trägerfolie nach unten. Das Motiv muss gespiegelt geschnitten werden – im Schneiden-Dialog **Spiegeln** einschalten:

- **Ganze Folie**: alles seitenverkehrt.
- **Jedes Element**: jedes Teil an seiner eigenen Mittelachse, die Anordnung bleibt.`,
  },
  {
    id: "large",
    heading: `Große Motive und Rolle`,
    text: `Unten unter **Fläche** gibt es **Eigene Größe** und **Rolle ohne Matte**.

- Ist das Motiv größer als die Matte, teilt der Schneiden-Dialog es in **Kacheln**. Kachel für Kachel schneiden, nach jeder neues Material einlegen. Eine kleine Überlappung erleichtert das Zusammensetzen.
- **Ohne Matte** nur mit Folie mit festem Trägerpapier. Am Ende wird die Folie hinter das Motiv geschoben.
- Für die Cameo gibt es die Matten 12″ × 12″ und 12″ × 24″.`,
  },
  {
    id: "draw",
    heading: `Formen, Text und Hilfslinien`,
    text: `Werkzeuge links: Rechteck, Ellipse, Vieleck, Stern, Herz, Linie, Freihand und Text. \`Umschalt\` beim Zeichnen hält die Form unverzerrt. Parameter wie Eckenradius oder Zackenzahl lassen sich danach im Objekt-Tab ändern.

- **Text**: Schrift, Größe, Abstand; **Bogen** setzt den Text auf einen Kreis. Text und Form zusammen auswählen → **Text auf Pfad**.
- **Hilfslinien** aus den Linealen ziehen; zurück aufs Lineal ziehen oder Rechtsklick löscht sie.
- **Raster** (oben): Verschieben rastet am Raster ein, Drehen in Winkelschritten. \`Alt\` beim Ziehen = frei.
- Breite und Höhe stehen am Objekt – anklicken zum Eintippen.
- Mehrere Objekte: Größe und Drehung für alle setzen, **verteilen** mit gleichen Abständen.
- **Rechtsklick** auf ein Objekt öffnet ein Menü mit den häufigsten Aktionen.
- Doppelklick auf einen Pfad: **Knoten bearbeiten**. **Knoten reduzieren** macht Pfade schlanker.`,
  },
  {
    id: "device",
    heading: `Plotter verbinden`,
    text: `**USB**: Plotter anschließen und einschalten – er wird automatisch erkannt. Beim .deb ist die nötige udev-Regel dabei (Plotter einmal neu einstecken). Beim Flatpak die Regel einmalig mit dem Befehl aus der README einrichten.

**Bluetooth**: oben auf den Plotter-Status klicken → **Per Bluetooth suchen** → verbinden. Schnittwerk verbindet sich danach automatisch wieder.

Im selben Menü steht der **Messer-Zähler**: geschnittene Strecke seit dem letzten Messerwechsel, mit Erinnerung.`,
  },
  {
    id: "keys",
    heading: `Tastenkürzel`,
    text: `- \`V\` Auswahl, \`H\` / \`Leertaste\` Ansicht verschieben
- \`R\` Rechteck, \`E\` Ellipse, \`P\` Vieleck, \`S\` Stern, \`L\` Linie, \`F\` Freihand, \`T\` Text
- \`Strg+Z\` / \`Strg+Y\` Rückgängig / Wiederholen (der Tooltip nennt den Schritt)
- \`Strg+C\` / \`Strg+X\` / \`Strg+V\` Kopieren / Ausschneiden / Einfügen
- \`Strg+D\` Duplizieren, \`Entf\` Löschen, \`Strg+G\` Gruppieren
- \`Strg+0\` Matte einpassen, \`Strg+2\` Auswahl einpassen, Mausrad = Zoom
- \`Strg+S\` Speichern, \`Strg+O\` Öffnen, \`Strg+N\` Startseite, \`Strg+I\` Importieren
- \`Strg+Enter\` Schneiden, \`Strg+P\` Drucken, \`Strg+Q\` Beenden
- \`F1\` Hilfe, \`Alt\`+Buchstabe öffnet ein Menü
- Pfeiltasten verschieben (mit \`Umschalt\` 10 mm, mit \`Alt\` 0,1 mm)`,
  },
  {
    id: "trouble",
    heading: `Probleme lösen`,
    text: `- **Plotter wird nicht gefunden**: Kabel und Einschalter prüfen; beim Flatpak die udev-Regel einrichten und den Plotter neu einstecken.
- **Passermarken werden nicht gefunden**: Bogen genau oben links anlegen, Druck ohne Skalierung, gutes Licht auf den Marken.
- **Schnitt geht nicht durch / reißt**: Testreihe machen, Messer prüfen (Messer-Zähler), Durchgänge erhöhen.
- **Druck und Schnitt verschoben**: Feinjustierung mit dem Testbogen.
- **App wird langsam**: sehr fein getracte Pfade mit **Knoten reduzieren** vereinfachen.

Fragen und Fehler gerne als Issue auf GitHub melden.`,
  },
];

export const DOCS_EN: DocSection[] = [
  {
    id: "start",
    heading: `Getting started`,
    text: `At start the **start page** opens (also by clicking “Schnittwerk” at the top left). Choose what you want to make – stickers, vinyl, heat transfer vinyl, paper, pen or a large design. Schnittwerk then sets material, mat and page accordingly.

- Load pictures and vectors with **Import** (\`Ctrl+I\`) or drop them onto the work area.
- On the right, the **Trace** tab turns a picture into cut lines, the **Sticker** tab into a sticker outline.
- **Cut** (\`Ctrl+Enter\`) takes you to the cutter.
- The **menu bar** at the very top holds every function – also by keyboard with \`Alt\` and the underlined letter. **File → Settings** has language, theme, grid and more.

Projects are saved as \`.schnitt\` files with the pictures inside. Unsaved work is backed up every 20 seconds and offered on the start page after a crash.`,
  },
  {
    id: "trace",
    heading: `Tracing pictures`,
    text: `Select a picture and open the **Trace** tab – the preview updates with every setting.

- **B/W**: dark areas become shapes – for drawings, lettering, logos.
- **Colour**: pick one colour with the picker.
- **Multicolour**: one layer per colour.
- **Cut out**: outline of PNGs with transparency.
- **Line**: one line through the middle of each stroke – ideal for the pen.

**Smoothing** calms the curves, **Remove specks** drops crumbs. Photos of drawings can be straightened with **Correct perspective** and freed from shadows with **Even out lighting**. **Eraser** and **Brush** clean up the picture right before tracing.`,
  },
  {
    id: "sticker",
    heading: `Stickers and Print & Cut`,
    text: `Stickers are printed and then cut out. The page gets **registration marks** that the cutter finds with its sensor before cutting.

- Switch on **Print & Cut** at the top (or choose “Sticker” on the start page).
- Select a picture, **Sticker** tab: remove the background or use transparency, set the margin, **Apply outline**.
- **Several pictures at once**: select them all – the Sticker tab shows **Create sticker sheet**. Every picture gets an outline, several copies if you like, arranged compactly.
- **Cut out individually** also cuts through with a backing margin (blue), otherwise kiss cut only (red).
- Print (\`Ctrl+P\`) – never choose “fit to page” in the print dialog.
- Put the sheet at the top left of the mat, load it, cut.

If the cut is slightly off, adjust it with the test sheet under **Fine adjustment** in the cut dialog.`,
  },
  {
    id: "cut",
    heading: `Cutting`,
    text: `In the cut dialog every **cut colour** gets its own material: red = kiss cut, blue = cut through, green = draw. Lighter cuts run first so the sheet holds together.

- Choose the **material**; adjust the values if needed and save them as a custom material.
- **Test cut**: a small square at the given position.
- **Test series**: several squares with increasing force or blade depth – then click the cleanest one and its value is applied.
- **Simulation**: shows beforehand, in cutting order, what the cutter will do.
- The **duration** is estimated and gets more accurate with every cut.
- After **cancelling** or a connection error, **Continue cutting** appears – the cut resumes where it stopped. Do not move the mat.

When a cut finishes while the window is in the background, Schnittwerk shows a notification; the sound can be switched off in the cutter menu.`,
  },
  {
    id: "vinyl",
    heading: `Vinyl and heat transfer vinyl`,
    text: `**Vinyl**: put it on the mat with the backing paper facing down. After cutting, the **weeding lines** (Object tab) help: a frame and lines through the waste so the vinyl peels off in strips.

**Heat transfer vinyl (textiles)**: shiny carrier facing down. The design must be cut mirrored – switch on **Mirror** in the cut dialog:

- **Whole sheet**: everything reversed.
- **Each element**: every part at its own centre line, the layout stays.`,
  },
  {
    id: "large",
    heading: `Large designs and rolls`,
    text: `At the bottom under **Area** there are **Custom size** and **Roll without mat**.

- If the design is larger than the mat, the cut dialog splits it into **tiles**. Cut tile by tile and load new material after each one. A small overlap makes putting them together easier.
- **No mat** only for material with a firm backing. At the end the material is pushed out past the design.
- For the Cameo there are the 12″ × 12″ and 12″ × 24″ mats.`,
  },
  {
    id: "draw",
    heading: `Shapes, text and guides`,
    text: `Tools on the left: rectangle, ellipse, polygon, star, heart, line, freehand and text. \`Shift\` while drawing keeps the shape undistorted. Parameters such as corner radius or number of points can be changed afterwards in the Object tab.

- **Text**: font, size, spacing; **Arc** puts the text on a circle. Select text and a shape together → **Text on path**.
- Drag **guides** out of the rulers; drag them back or right-click to delete them.
- **Grid** (top): moving snaps to the grid, rotating to angle steps. \`Alt\` while dragging = free.
- Width and height are shown at the object – click to type.
- Several objects: set size and rotation for all, **distribute** with equal gaps.
- **Right-click** an object for a menu with the most common actions.
- Double-click a path: **Edit nodes**. **Reduce nodes** makes paths lighter.`,
  },
  {
    id: "device",
    heading: `Connecting the cutter`,
    text: `**USB**: connect the cutter and switch it on – it is detected automatically. The .deb includes the required udev rule (plug the cutter in again once). For the Flatpak, set up the rule once with the command from the README.

**Bluetooth**: click the cutter status at the top → **Search via Bluetooth** → connect. Schnittwerk then reconnects automatically.

The same menu shows the **blade counter**: distance cut since the last blade change, with a reminder.`,
  },
  {
    id: "keys",
    heading: `Keyboard shortcuts`,
    text: `- \`V\` select, \`H\` / \`Space\` pan
- \`R\` rectangle, \`E\` ellipse, \`P\` polygon, \`S\` star, \`L\` line, \`F\` freehand, \`T\` text
- \`Ctrl+Z\` / \`Ctrl+Y\` undo / redo (the tooltip names the step)
- \`Ctrl+C\` / \`Ctrl+X\` / \`Ctrl+V\` copy / cut / paste
- \`Ctrl+D\` duplicate, \`Del\` delete, \`Ctrl+G\` group
- \`Ctrl+0\` fit mat, \`Ctrl+2\` fit selection, mouse wheel = zoom
- \`Ctrl+S\` save, \`Ctrl+O\` open, \`Ctrl+N\` start page, \`Ctrl+I\` import
- \`Ctrl+Enter\` cut, \`Ctrl+P\` print, \`Ctrl+Q\` quit
- \`F1\` help, \`Alt\`+letter opens a menu
- Arrow keys move (with \`Shift\` 10 mm, with \`Alt\` 0.1 mm)`,
  },
  {
    id: "trouble",
    heading: `Troubleshooting`,
    text: `- **Cutter not found**: check cable and power switch; for the Flatpak, set up the udev rule and plug the cutter in again.
- **Registration marks not found**: put the sheet exactly at the top left, print without scaling, good light on the marks.
- **Cut does not go through / tears**: make a test series, check the blade (blade counter), add passes.
- **Print and cut offset**: fine adjustment with the test sheet.
- **App gets slow**: simplify very finely traced paths with **Reduce nodes**.

Questions and bugs are welcome as an issue on GitHub.`,
  },
];

/** Light markup → HTML (text is escaped first). */
export function renderDoc(text: string): string {
  const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const inline = (s: string) =>
    esc(s)
      .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
      .replace(/`(.+?)`/g, "<kbd>$1</kbd>");
  return text
    .split(/\n\s*\n/)
    .map((block) => {
      const lines = block.split("\n");
      if (lines.every((l) => l.trim().startsWith("- "))) {
        return "<ul>" + lines.map((l) => `<li>${inline(l.trim().slice(2))}</li>`).join("") + "</ul>";
      }
      return `<p>${inline(block.replace(/\n/g, " "))}</p>`;
    })
    .join("");
}
