import { getT } from "../../i18n";
import { client } from "../../ipc/client";
import { askPath, useHelp, type PathRequest } from "./state";

interface FileRequest extends PathRequest {
  kind: "open" | "save";
  extensions: string[];
  defaultPath?: string;
}

/** Desktop uses the installed dialog plugin; the path form is only a browser/bridge fallback. */
export async function chooseFile(request: FileRequest, active = () => true): Promise<void> {
  if (client.transportName !== "tauri") {
    askPath(request);
    return;
  }
  const path = await client.invoke<string | null>(`dialog.${request.kind}`, { options: {
    title: getT()(request.title, undefined, { fallback: request.titleFallback }),
    filters: [{ name: request.extensions.map(ext => `*.${ext}`).join(", "), extensions: request.extensions }],
    ...(request.kind === "open" ? { directory: false, multiple: false } : {}),
    ...(request.defaultPath ? { defaultPath: request.defaultPath } : {}),
  } });
  if (!path || !active()) return;
  const operation = await request.submit(path);
  if (active() && operation) useHelp.setState({ operation });
}
