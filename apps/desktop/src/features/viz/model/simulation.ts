// Auswahl der Simulation nach Dimension (2D: Quadtree, UI-092; 3D: Octree, UI-098) für Worker und Hauptthread.
import { ForceSim, type ForceInput, type Simulation } from "./force";
import { ForceSim3D } from "./force3d";

export function createSimulation(input: ForceInput): Simulation {
  return input.dims === 3 ? new ForceSim3D(input) : new ForceSim(input);
}
