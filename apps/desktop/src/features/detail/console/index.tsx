import type { DetailPlugin } from "../frame/types";
import { ConsoleLog } from "./ConsoleLog";

export const detailTab: DetailPlugin = { id: "consoleLog", Component: ConsoleLog };
