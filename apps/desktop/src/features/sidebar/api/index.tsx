import type { SidebarPanel } from "../frame/SidebarFrame";
import { ApiPanel } from "./ApiPanel";

/** Contributes the API tab to the sidebar frame (UI-070). */
export const panel: SidebarPanel = { id: "api", titleKey: "sidebar-tab-api", Component: ApiPanel };
