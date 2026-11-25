// First-start tour: a few coach marks pointing at the main controls.

const KEY = "schnittwerk.tourDone";

function loadDone(): boolean {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return true;
  }
}

export const tour = $state({ step: -1, done: loadDone() });

export interface TourStep {
  /** data-tour value of the element to point at. */
  target: string;
  title: string;
  text: string;
}

export const TOUR_STEPS: TourStep[] = [
  { target: "home", title: "Startseite", text: "Ein Klick auf das Logo bringt dich zurück zur Startseite: neues Projekt, Vorlagen und zuletzt geöffnete Dateien." },
  { target: "menu", title: "Menüleiste", text: "Alle Funktionen an einem Ort: Datei, Bearbeiten, Objekt, Ansicht, Plotter und Hilfe – mit Alt und dem unterstrichenen Buchstaben auch per Tastatur." },
  { target: "save", title: "Speichern", text: "Speichert das Projekt samt Bildern. Ein Punkt zeigt ungespeicherte Änderungen; eine Sicherheitskopie entsteht zusätzlich automatisch." },
  { target: "import", title: "Importieren", text: "Bilder, SVG und PDF laden – oder einfach auf die Arbeitsfläche ziehen." },
  { target: "tools", title: "Werkzeuge", text: "Auswählen, Formen, Linien, Freihand und Text. Rechtsklick auf ein Objekt zeigt die häufigsten Aktionen." },
  { target: "undo", title: "Rückgängig", text: "Macht Schritte rückgängig oder wiederholt sie. Der Tooltip nennt den Schritt, zum Beispiel „Rückgängig: Verschieben“." },
  { target: "modes", title: "Raster und Print & Cut", text: "Raster lässt Objekte einrasten. Print & Cut legt Passermarken für Sticker auf die Seite." },
  { target: "tabs", title: "Rechte Seite", text: "Objekt bearbeiten, Bilder tracen, Sticker-Konturen erzeugen und Ebenen verwalten." },
  { target: "area", title: "Fläche", text: "Matte, Papierformat, Rolle ohne Matte oder eine eigene Größe für große Motive." },
  { target: "device", title: "Plotter", text: "Zeigt, ob der Plotter verbunden ist. Hier verbindest du per Bluetooth und siehst den Messer-Zähler." },
  { target: "print", title: "Drucken", text: "Für Print & Cut: druckt die Seite mit Passermarken ohne Skalierung oder speichert sie als PDF." },
  { target: "cut", title: "Schneiden", text: "Material wählen, Testschnitt machen, Ablauf simulieren und an den Plotter senden." },
  { target: "help", title: "Hilfe", text: "Anleitungen zu allen Themen und die Tastenkürzel. Die Einführung lässt sich dort jederzeit neu starten." },
];

export function startTour() {
  tour.step = 0;
}

export function endTour() {
  tour.step = -1;
  tour.done = true;
  try {
    localStorage.setItem(KEY, "1");
  } catch {}
}
