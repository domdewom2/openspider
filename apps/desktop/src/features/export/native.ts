import { join, dirname } from "@tauri-apps/api/path";
import { client } from "../../ipc/client";
import { IpcError, localError } from "../../ipc/errors";

export interface DestinationOptions {
  title: string;
  defaultName: string;
  directory: string | null;
  extension: string;
  folder: boolean;
}
export interface ExportDesktop {
  choose(options: DestinationOptions): Promise<string | null>;
  directory(path: string, folder: boolean): Promise<string>;
  reveal(path: string): Promise<void>;
}

/** Native plugin dialogs perform overwrite confirmation before any export command is issued. */
export const nativeExportDesktop: ExportDesktop = {
  async choose(options) {
    if (client.transportName !== "tauri") throw new IpcError(localError("Unsupported", "export-native-required"));
    const defaultPath = options.directory
      ? options.folder ? options.directory : await join(options.directory, options.defaultName)
      : options.folder ? undefined : options.defaultName;
    if (options.folder) {
      return client.invoke<string | null>("dialog.open", { options: { title: options.title, directory: true, multiple: false, defaultPath } });
    }
    return client.invoke<string | null>("dialog.save", {
      options: { title: options.title, defaultPath, filters: [{ name: options.extension.toUpperCase(), extensions: [options.extension] }] },
    });
  },
  directory: (path, folder) => folder ? Promise.resolve(path) : dirname(path),
  reveal: (path) => client.invoke<void>("open_export_folder", { path }),
};
