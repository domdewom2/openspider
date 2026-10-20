import { create } from "zustand";
import type { CrawlModeDto } from "../../ipc/gen/CrawlModeDto";
import type { OpHandle } from "../../ipc/gen/OpHandle";

export type HelpPage = "shortcuts" | "about" | "debug" | "usage";
export interface PathRequest {
  title: string;
  titleFallback?: string;
  directory?: boolean;
  submit(path: string): Promise<OpHandle | void>;
}
interface HelpState {
  page: HelpPage | null;
  path: PathRequest | null;
  message: string | null;
  operation: OpHandle | null;
}
export const useHelp = create<HelpState>(() => ({ page: null, path: null, message: null, operation: null }));
export const showHelp = (page: HelpPage) => useHelp.setState({ page, message: null });
export const askPath = (path: PathRequest) => useHelp.setState({ path, message: null });

/** N01 integrates this mode selection with U03 useHeader.mode; CrawlConfig knows only Spider/List. */
export const useMainMode = create<{ mode: CrawlModeDto }>(() => ({ mode: "spider" }));
