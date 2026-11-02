export function rendererRss(rootPid?: number): {
  scope: string;
  cpuMs: number | null;
  bytes: number;
  processes: { pid: number; bytes: number }[];
};
