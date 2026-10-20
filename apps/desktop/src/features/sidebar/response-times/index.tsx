import type { SidebarPanel } from "../frame/SidebarFrame";
import { ResponseTimes } from "./ResponseTimes";

/** Contributes the Response Times tab to the sidebar frame (UI-069). */
export const panel: SidebarPanel = { id: "responseTimes", titleKey: "sidebar-tab-responseTimes", Component: ResponseTimes };
