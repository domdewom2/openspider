// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { SetIntervalArgs } from "./gen/SetIntervalArgs";
import type { SetVisibilityArgs } from "./gen/SetVisibilityArgs";
import type { SubscribeArgs } from "./gen/SubscribeArgs";
import type { SubscriptionId } from "./gen/SubscriptionId";
import { client } from "./client";

export const events = {
  /** `events.subscribe` */
  subscribe: (args: SubscribeArgs, signal?: AbortSignal) => client.invoke<SubscriptionId>("events.subscribe", args, signal),
  /** `events.setVisibility` */
  setVisibility: (args: SetVisibilityArgs, signal?: AbortSignal) => client.invoke<void>("events.setVisibility", args, signal),
  /** `events.setInterval` */
  setInterval: (args: SetIntervalArgs, signal?: AbortSignal) => client.invoke<void>("events.setInterval", args, signal),
} as const;
