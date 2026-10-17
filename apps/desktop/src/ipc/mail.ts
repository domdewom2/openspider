// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { IdArgs } from "./gen/IdArgs";
import type { MailAccountInfo } from "./gen/MailAccountInfo";
import type { MailSaveAccountArgs } from "./gen/MailSaveAccountArgs";
import type { MailTestArgs } from "./gen/MailTestArgs";
import type { OpHandle } from "./gen/OpHandle";
import { client } from "./client";

export const mail = {
  /** `mail.listAccounts` */
  listAccounts: (signal?: AbortSignal) => client.invoke<MailAccountInfo[]>("mail.listAccounts", undefined, signal),
  /** `mail.saveAccount` */
  saveAccount: (args: MailSaveAccountArgs, signal?: AbortSignal) => client.invoke<MailAccountInfo>("mail.saveAccount", args, signal),
  /** `mail.deleteAccount` */
  deleteAccount: (args: IdArgs, signal?: AbortSignal) => client.invoke<void>("mail.deleteAccount", args, signal),
  /** `mail.testAccount` – lang laufend: liefert sofort `OpHandle`, Ergebnis (MailTestResult) über `Tick.ops[].result`. */
  testAccount: (args: MailTestArgs, signal?: AbortSignal) => client.invoke<OpHandle>("mail.testAccount", args, signal),
} as const;
