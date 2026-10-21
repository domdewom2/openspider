// Kontextaktionen des Rechtschreib-Tabs (ONPAGE-157). „Regel ignorieren“ schaltet die Grammatikregel in der Crawl-Konfiguration ab;
// „Ignorieren (alle)“ und „Zum Wörterbuch hinzufügen“ haben im Command-Katalog noch keinen Eintrag (Auflage an K09/D19) und melden bis dahin
// `Unsupported`. Tests und die Anwendung können die Umsetzung über `setSpellingActions` ersetzen.
import { config } from "../../../ipc/config";
import { IpcError, localError } from "../../../ipc/errors";

export interface SpellingActions {
  ignoreWord(word: string, language: string | undefined): Promise<void>;
  ignoreRule(ruleId: string): Promise<void>;
  addToDictionary(word: string, language: string | undefined): Promise<void>;
}
const unsupported = () => new IpcError(localError("Unsupported", "err-unsupported"));
export const defaultSpellingActions: SpellingActions = {
  async ignoreWord() { throw unsupported(); },
  async addToDictionary() { throw unsupported(); },
  async ignoreRule(ruleId) {
    const dto = await config.get();
    const cfg = structuredClone(dto.config) as { grammar?: { disabled_rules?: string[] } };
    const grammar = cfg.grammar ??= {}; const rules = grammar.disabled_rules ??= [];
    if (!rules.includes(ruleId)) rules.push(ruleId);
    await config.set({ ...dto, config: cfg as typeof dto.config });
  },
};
let active: SpellingActions = defaultSpellingActions;
export const spellingActions = (): SpellingActions => active;
export function setSpellingActions(next: SpellingActions | undefined): void { active = next ?? defaultSpellingActions; }
