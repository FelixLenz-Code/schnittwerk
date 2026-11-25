# Roadmap

✅ erledigt · 🚧 in Arbeit · ⬜ geplant

## Etappe 1 – Grundgerüst
- ✅ Tauri-App, modernes Hell/Dunkel-Design (folgt dem System, umschaltbar)
- ✅ Arbeitsfläche mit Matte (Portrait 3 8″×12″, A4, Letter), Raster, Linealen
- ✅ Zoom am Mauszeiger, Verschieben (Hand, Leertaste, mittlere Maustaste)
- ✅ Auswählen, Rahmenauswahl, Verschieben, Skalieren, Drehen, Spiegeln
- ✅ Ausrichten, Anordnen, Duplizieren, Sperren, Ein-/Ausblenden, Ebenenliste
- ✅ Rückgängig/Wiederholen
- ✅ Import: PNG, JPG, WebP, BMP, GIF, TIFF, SVG, PDF, DXF; Drag & Drop
- ✅ Projektdateien (.schnitt, Bilder eingebettet), SVG-Export der Schnittlinien
- ✅ Rechteck- und Ellipsen-Werkzeug
- ✅ Einfügen aus der Zwischenablage (Bilder, SVG), Kopieren/Ausschneiden/Einfügen von Objekten
- ✅ PDF-Import (Vektoren 1:1, Scans als Bild) und DXF-Import (Linien, Polylinien mit Bögen, Kreise, Ellipsen, Splines, Blöcke)

## Etappe 2 – Tracen
- ✅ Live-Vorschau beim Verstellen der Regler
- ✅ Schwarz/Weiß mit automatischem Schwellwert (Otsu), Invertieren
- ✅ Farbe per Pipette mit Toleranz
- ✅ Mehrfarbig (k-means, Hintergrund weglassen) → eine Ebene je Farbe
- ✅ Freistellen (Umriss nicht-transparenter Pixel)
- ✅ Helligkeit, Kontrast, Weichzeichnen
- ✅ Eigene Kontur-Pipeline: Subpixel-Konturen, Ecken-Rekonstruktion, Bézier-Fitting (Schneider)
- ✅ Glättung, Flecken entfernen, Dicker/Dünner, Nur Außenkontur, Kurven/Ecken
- ✅ Auflösung wählbar (schnell → Original)
- ✅ Radierer/Pinsel zum Bereinigen vor dem Tracen (rückgängig machbar, Live-Neutracen)
- ✅ Perspektivkorrektur (Blatt automatisch erkennen, Ecken nachziehen, Papierformat in Originalgröße) und Beleuchtungsausgleich für Fotos von Zeichnungen
- ✅ Mittellinien-Modus: eine Linie mitten durch jeden Strich (Skelett, Sporn-Entfernung, Kurvenfit)

## Etappe 3 – Plotter
- ✅ USB-Erkennung Portrait 3 (rusb), udev-Regel
- ✅ GPGL-Befehle: Matte, Druck, Geschwindigkeit, AutoBlade-Tiefe, Bewegen/Schneiden (durchgehendes Senden ohne Stocken)
- ✅ Schnittoptimierung: innen vor außen, kurze Wege, Überschnitt (Messerversatz macht das Gerät, FC)
- ✅ Mehrfachdurchgänge, Testschnitt, Stiftmodus, Spiegeln für Flexfolie
- ✅ Materialbibliothek mit Vorlagen und eigenen Profilen
- ✅ Vorschau der Schnittreihenfolge, Fortschritt (je Schritt, auch während der Plotter den Puffer abarbeitet), Pause, Abbrechen
- ✅ Kiss-Cut und Durchschnitt in einem Auftrag: Material je Schnittfarbe, leichte Durchgänge zuerst, Schnittrahmen
- ⬜ Prägen, Perforation

## Etappe 4 – Print & Cut
- ✅ Sticker-Assistent: Hintergrund entfernen oder Transparenz → Rand in mm → glatte Kontur (Live-Vorschau)
- ✅ Einzelsticker automatisch: anschneiden (Kiss-Cut, abziehbar) + mit Trägerrand durchschneiden
- ✅ Konturversatz für Pfade (innen/außen, Teile verschmelzen) per Distanztransformation
- ✅ Print & Cut-Seite (A4/Letter) mit Silhouette-Passermarken (Typ 1) und gesperrten Zonen
- ✅ Bogen füllen (Raster, Abstand einstellbar, Markenzonen werden ausgespart), Gruppieren
- ✅ Rand farbig drucken, Pfade „nur drucken“ oder „nur schneiden“
- ✅ Drucken über CUPS ohne Skalierung (300 dpi) oder als PDF speichern
- ✅ Passermarken-Suche mit optischem Sensor (TB123, Werte nach py_silhouette)
- ✅ Bleed (Randfarben des Motivs über die Schnittlinie hinaus)
- ✅ Feinjustierung Druck ↔ Schnitt mit Testbogen
- ✅ PixScan-Matte: Foto der Matte → Innenrahmen und Ausrichtung automatisch erkennen (Ecken notfalls von Hand), in Originalgröße an die Schneideposition legen; Kalibrierung mit Testkreuzen

## Etappe 5 – Ausbau
- ✅ Text mit Systemschriften (Suche, fett/kursiv, Abstand, mehrzeilig, Ausrichtung), Verschweißen
- ✅ Boolesche Operationen: Vereinen, Abziehen, Schnittmenge, Ausschließen (i_overlay)
- ✅ Knotenbearbeitung: Knoten/Griffe ziehen, einfügen (Doppelklick), löschen, Ecke/Glatt, Gerade/Kurve
- ✅ Entgitterungshilfen: Rahmen und Linien durch den Abfall (mit Abstand zum Motiv, auch durch Innenräume)
- ✅ .deb-Paket mit Menüeintrag, udev-Regel und Abhängigkeiten
- ✅ Nesting: echte Umrisse verschachteln und drehen (Füllen mit Kopien oder Anordnen)
- ✅ Vorlagen: Sticker/Etiketten (Print & Cut), Klappkarte, Lesezeichen, Geschenkanhänger
- ✅ CLI-Modus (`schnittwerk-cli`): tracen, SVG/DXF/Projekte schneiden, USB und Bluetooth

## Etappe 6 – Anbindungen
- ✅ Bluetooth LE: Suchen, Verbinden (automatisches Wiederverbinden), Schneiden über den Silhouette-GATT-Dienst – an der Portrait 3 getestet;
  gleiche GPGL-Befehle, nur anderer Transport
- ✅ MCP-Schnittstelle (`schnittwerk-mcp`): Bilder tracen, Sticker-Bögen als Projekt anlegen,
  Schnitt-Vorschau und Schneiden nur mit Bestätigung; im .deb enthalten

## Etappe 7 – Komfort
- ✅ Assistent für neue Projekte: Projektart (Sticker, Vinyl, Flexfolie, Papier, Stift, großes Motiv),
  Material, Matte oder Rolle, Druckseite, Sticker-Art, Vorlage; beim Start mit „Zuletzt geöffnet“
- ✅ Automatisches Speichern (Sicherheitskopie alle 20 s) und Wiederherstellen nach einem Absturz
- ✅ Hilfslinien aus den Linealen, Einrasten an Hilfslinien, Matte und Objekten
- ✅ Maße direkt am Objekt, per Klick editierbar
- ✅ Formen: abgerundetes Rechteck, Vieleck, Stern, Herz, Linie, Freihand – Parameter bleiben änderbar
- ✅ Text auf einem Bogen oder entlang eines beliebigen Pfads
- ✅ Pfad vereinfachen
- ✅ Große Motive in Kacheln schneiden, Schneiden ohne Matte von der Rolle (mit Vorschub am Ende)
- ✅ Messerverschleiß-Zähler
- ✅ Nachfrage beim Schließen mit ungespeicherten Änderungen
- ✅ Flexfolie: ganze Folie oder jedes Element einzeln spiegeln
- ✅ Raster beim Verschieben und Winkelschritte beim Drehen (abschaltbar, standardmäßig an)

## Etappe 8 – Schneiden und Sprache
- ✅ Geschätzte Schnittzeit und Restzeit (selbstlernend)
- ✅ Weiterschneiden nach Abbruch oder Verbindungsfehler (ab dem ersten nicht sicher geschnittenen Pfad, verstellbar)
- ✅ Matten 12″ × 12″ und 12″ × 24″ für die Cameo
- ✅ Englische Oberfläche (Sprache folgt dem System, umschaltbar)
