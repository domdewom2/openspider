// Web Worker des kraftgerichteten Layouts (UI-146: Layout im Worker, blockiert die Oberfläche nicht).
import { ForceDriver, type ForceReply, type ForceRequest } from "./force";

const scope = self as unknown as { postMessage(msg: ForceReply, transfer: Transferable[]): void; onmessage: ((e: MessageEvent<ForceRequest>) => void) | null };
const driver = new ForceDriver((reply) => scope.postMessage(reply, [reply.positions.buffer]));
scope.onmessage = (e) => driver.handle(e.data);
