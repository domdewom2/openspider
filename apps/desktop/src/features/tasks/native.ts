import { client } from "../../ipc/client";
import { IpcError, localError } from "../../ipc/errors";

export interface TaskDesktop { choose(kind: "import" | "export"): Promise<string | null> }
export const taskDesktop: TaskDesktop = {
  async choose(kind) {
    if (client.transportName !== "tauri") throw new IpcError(localError("Unsupported", "export-native-required"));
    return client.invoke<string | null>(kind === "import" ? "dialog.open" : "dialog.save", {
      options: { multiple: false, directory: false, filters: [{ name: "OpenSpider Tasks", extensions: ["ostask"] }] },
    });
  },
};
