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
- ⬜ Perspektivkorrektur und Beleuchtungsausgleich für Fotos von Zeichnungen
- ✅ Mittellinien-Modus: eine Linie mitten durch jeden Strich (Skelett, Sporn-Entfernung, Kurvenfit)

## Etappe 3 – Plotter
- ✅ USB-Erkennung Portrait 3 (rusb), udev-Regel
- ✅ GPGL-Befehle: Matte, Druck, Geschwindigkeit, AutoBlade-Tiefe, Bewegen/Schneiden (durchgehendes Senden ohne Stocken)
- ✅ Schnittoptimierung: innen vor außen, kurze Wege, Überschnitt (Messerversatz macht das Gerät, FC)
- ✅ Mehrfachdurchgänge, Testschnitt, Stiftmodus, Spiegeln für Flexfolie
- ✅ Materialbibliothek mit Vorlagen und eigenen Profilen
- ✅ Vorschau der Schnittreihenfolge, Fortschritt, Pause, Abbrechen
- ✅ Kiss-Cut und Durchschnitt in einem Auftrag: Material je Schnittfarbe, leichte Durchgänge zuerst, Schnittrahmen
- ⬜ Prägen, Perforation

## Etappe 4 – Print & Cut
- ✅ Sticker-Assistent: Hintergrund entfernen oder Transparenz → Rand in mm → glatte Kontur (Live-Vorschau)
- ✅ Konturversatz für Pfade (innen/außen, Teile verschmelzen) per Distanztransformation
- ✅ Print & Cut-Seite (A4/Letter) mit Silhouette-Passermarken (Typ 1) und gesperrten Zonen
- ✅ Bogen füllen (Raster, Abstand einstellbar, Markenzonen werden ausgespart), Gruppieren
- ✅ Rand farbig drucken, Pfade „nur drucken“ oder „nur schneiden“
- ✅ Drucken über CUPS ohne Skalierung (300 dpi) oder als PDF speichern
- ✅ Passermarken-Suche mit optischem Sensor (TB123, Werte nach py_silhouette)
- ✅ Bleed (Randfarben des Motivs über die Schnittlinie hinaus)
- ✅ Feinjustierung Druck ↔ Schnitt mit Testbogen

## Etappe 5 – Ausbau
- ✅ Text mit Systemschriften (Suche, fett/kursiv, Abstand, mehrzeilig, Ausrichtung), Verschweißen
- ✅ Boolesche Operationen: Vereinen, Abziehen, Schnittmenge, Ausschließen (i_overlay)
- ⬜ Knotenbearbeitung, Entgitterungshilfen
- ✅ .deb-Paket mit Menüeintrag, udev-Regel und Abhängigkeiten
- ✅ Nesting: echte Umrisse verschachteln und drehen (Füllen mit Kopien oder Anordnen)
- ⬜ Vorlagen, CLI-Modus

## Etappe 6 – Anbindungen
- 🚧 Bluetooth LE: Suchen, Verbinden, Schneiden über den Silhouette-GATT-Dienst – implementiert, Hardware-Test ausstehend;
  gleiche GPGL-Befehle, nur anderer Transport
- ⬜ MCP-Schnittstelle: Schnittwerk als MCP-Server, damit KI-Assistenten (z. B. Claude) Bilder tracen,
  Sticker-Bögen anlegen, die Vorschau prüfen und Schnitte starten können – mit Bestätigung vor dem Schneiden

