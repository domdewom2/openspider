// Web Worker des kraftgerichteten Layouts (UI-146: Layout im Worker, blockiert die Oberfläche nicht); 2D und 3D.
import { ForceDriver, type ForceReply, type ForceRequest } from "./force";
import { createSimulation } from "./simulation";

const scope = self as unknown as { postMessage(msg: ForceReply, transfer: Transferable[]): void; onmessage: ((e: MessageEvent<ForceRequest>) => void) | null };
const driver = new ForceDriver(
  (reply) => scope.postMessage(reply, reply.type === "done" ? [reply.positions.buffer, reply.velocities.buffer] : [reply.positions.buffer]),
  undefined,
  undefined,
  undefined,
  createSimulation,
);
scope.onmessage = (e) => driver.handle(e.data);
