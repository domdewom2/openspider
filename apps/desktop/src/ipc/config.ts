// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { ConfigDiffArgs } from "./gen/ConfigDiffArgs";
import type { ConfigSetResult } from "./gen/ConfigSetResult";
import type { CrawlConfigDto } from "./gen/CrawlConfigDto";
import type { DiffRow } from "./gen/DiffRow";
import type { JsonValue } from "./gen/serde_json/JsonValue";
import type { RobotsLoadArgs } from "./gen/RobotsLoadArgs";
import type { RobotsTestArgs } from "./gen/RobotsTestArgs";
import type { RobotsTestResult } from "./gen/RobotsTestResult";
import type { RuleTrace } from "./gen/RuleTrace";
import type { TestRulesArgs } from "./gen/TestRulesArgs";
import type { ValidationIssue } from "./gen/ValidationIssue";
import { client } from "./client";

export const config = {
  /** `config.get` */
  get: (signal?: AbortSignal) => client.invoke<CrawlConfigDto>("config.get", undefined, signal),
  /** `config.validate` */
  validate: (args: CrawlConfigDto, signal?: AbortSignal) => client.invoke<ValidationIssue[]>("config.validate", args, signal),
  /** `config.set` */
  set: (args: CrawlConfigDto, signal?: AbortSignal) => client.invoke<ConfigSetResult>("config.set", args, signal),
  /** `config.schema` */
  schema: (signal?: AbortSignal) => client.invoke<JsonValue>("config.schema", undefined, signal),
  /** `config.diff` */
  diff: (args: ConfigDiffArgs, signal?: AbortSignal) => client.invoke<DiffRow[]>("config.diff", args, signal),
  /** `config.testRules` */
  testRules: (args: TestRulesArgs, signal?: AbortSignal) => client.invoke<RuleTrace>("config.testRules", args, signal),
  /** `config.testRobots` */
  testRobots: (args: RobotsTestArgs, signal?: AbortSignal) => client.invoke<RobotsTestResult>("config.testRobots", args, signal),
  /** `config.loadRobots` */
  loadRobots: (args: RobotsLoadArgs, signal?: AbortSignal) => client.invoke<string>("config.loadRobots", args, signal),
} as const;
