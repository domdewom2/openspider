import { stripVTControlCharacters } from "node:util";

// CI forces terminal colors even on piped output; escape sequences can split
// both "Local:" and the port number. Match the accumulated, normalized output.
export function viteReady(output) {
  return /\bLocal:\s+http:\/\/localhost:1420\/(?:\s|$)/.test(stripVTControlCharacters(output));
}
