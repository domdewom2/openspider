import type { SidebarPanel } from "../frame/SidebarFrame";
import { SiteStructure } from "./SiteStructure";

/** Contributes the Site Structure tab to the sidebar frame (UI-066). */
export const panel: SidebarPanel = { id: "siteStructure", titleKey: "sidebar-tab-siteStructure", Component: SiteStructure };
