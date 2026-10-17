// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { RuleNode } from "./gen/RuleNode";
import type { SearchParseArgs } from "./gen/SearchParseArgs";
import type { ValidateRegexArgs } from "./gen/ValidateRegexArgs";
import { client } from "./client";

export const search = {
  /** `search.parse` */
  parse: (args: SearchParseArgs, signal?: AbortSignal) => client.invoke<RuleNode>("search.parse", args, signal),
  /** `search.validateRegex` */
  validateRegex: (args: ValidateRegexArgs, signal?: AbortSignal) => client.invoke<void>("search.validateRegex", args, signal),
} as const;
