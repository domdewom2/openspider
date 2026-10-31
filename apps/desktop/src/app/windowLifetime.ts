/** Page close must release React effects as well as the IPC runtime, including
 * engines which keep a closed WebView process in their page cache (UI-144). */
export function bindPageClose(dispose: () => void, target: Window = window): () => void {
  let disposed = false;
  const close = () => {
    if (disposed) return;
    disposed = true;
    target.removeEventListener("pagehide", close);
    dispose();
  };
  target.addEventListener("pagehide", close);
  return close;
}
