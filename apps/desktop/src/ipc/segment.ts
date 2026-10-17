// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { IdArgs } from "./gen/IdArgs";
import type { OpHandle } from "./gen/OpHandle";
import type { SegmentDef } from "./gen/SegmentDef";
import type { SegmentPreviewArgs } from "./gen/SegmentPreviewArgs";
import type { SegmentSaveArgs } from "./gen/SegmentSaveArgs";
import type { SegmentSaveResult } from "./gen/SegmentSaveResult";
import type { SegmentValidateArgs } from "./gen/SegmentValidateArgs";
import type { ValidationIssue } from "./gen/ValidationIssue";
import { client } from "./client";

export const segment = {
  /** `segment.list` */
  list: (signal?: AbortSignal) => client.invoke<SegmentDef[]>("segment.list", undefined, signal),
  /** `segment.save` */
  save: (args: SegmentSaveArgs, signal?: AbortSignal) => client.invoke<SegmentSaveResult>("segment.save", args, signal),
  /** `segment.delete` */
  delete: (args: IdArgs, signal?: AbortSignal) => client.invoke<void>("segment.delete", args, signal),
  /** `segment.validate` */
  validate: (args: SegmentValidateArgs, signal?: AbortSignal) => client.invoke<ValidationIssue[]>("segment.validate", args, signal),
  /** `segment.preview` – lang laufend: liefert sofort `OpHandle`, Ergebnis (SegmentPreview) über `Tick.ops[].result`. */
  preview: (args: SegmentPreviewArgs, signal?: AbortSignal) => client.invoke<OpHandle>("segment.preview", args, signal),
  /** `segment.reassign` – lang laufend: liefert sofort `OpHandle`, Ergebnis (CountResult) über `Tick.ops[].result`. */
  reassign: (signal?: AbortSignal) => client.invoke<OpHandle>("segment.reassign", undefined, signal),
} as const;
