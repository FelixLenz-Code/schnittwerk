// New-project assistant: open state and the "show at start" preference.

const KEY = "schnittwerk.wizardAtStart";

function loadAtStart(): boolean {
  try {
    return localStorage.getItem(KEY) !== "0";
  } catch {
    return true;
  }
}

export const wizardUi = $state({ open: false, atStart: loadAtStart() });

export function setWizardAtStart(v: boolean) {
  wizardUi.atStart = v;
  try {
    localStorage.setItem(KEY, v ? "1" : "0");
  } catch {}
}
