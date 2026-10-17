// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { CompareOpenArgs } from "./gen/CompareOpenArgs";
import type { CompareSession } from "./gen/CompareSession";
import type { RuleTrace } from "./gen/RuleTrace";
import type { TestMappingArgs } from "./gen/TestMappingArgs";
import { client } from "./client";

export const compare = {
  /** `compare.open` */
  open: (args: CompareOpenArgs, signal?: AbortSignal) => client.invoke<CompareSession>("compare.open", args, signal),
  /** `compare.swap` */
  swap: (signal?: AbortSignal) => client.invoke<CompareSession>("compare.swap", undefined, signal),
  /** `compare.testMapping` */
  testMapping: (args: TestMappingArgs, signal?: AbortSignal) => client.invoke<RuleTrace>("compare.testMapping", args, signal),
  /** `compare.close` */
  close: (signal?: AbortSignal) => client.invoke<void>("compare.close", undefined, signal),
} as const;
