import type { SidebarPanel } from "../frame/SidebarFrame";
import { Segments } from "./Segments";
import "./segments.css";

/** Contributes the Segments tab to the sidebar frame (UI-067). */
export const panel: SidebarPanel = { id: "segments", titleKey: "sidebar-tab-segments", Component: Segments };
