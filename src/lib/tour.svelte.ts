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
  { target: "area", title: "Matte und Papier", text: "Worauf schneidest du? Matte, Papier, Print & Cut mit Passermarken, Rolle ohne Matte oder eine eigene Größe für große Motive." },
  { target: "tabs", title: "Rechte Seite", text: "Zeigt immer, was zur Auswahl passt: Werkzeuge für den ganzen Bogen, wenn nichts ausgewählt ist, sonst das Objekt. Bei Bildern geht es von hier zum Tracen oder zum Sticker. Unten liegen die Ebenen." },
  { target: "view", title: "Ansicht", text: "Einrasten am Raster und an Hilfslinien, dazu der Zoom. Ein Klick auf die Prozentzahl passt die Matte ein." },
  { target: "device", title: "Plotter", text: "Zeigt, ob der Plotter verbunden ist. Hier verbindest du per Bluetooth und siehst den Messer-Zähler." },
  { target: "print", title: "Drucken", text: "Erscheint bei Print & Cut: druckt die Seite mit Passermarken ohne Skalierung oder speichert sie als PDF." },
  { target: "cut", title: "Schneiden", text: "Führt Schritt für Schritt durch den Schnitt: Plotter und Matte, Material, Testschnitt an einer Stelle deiner Wahl und dann schneiden." },
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
