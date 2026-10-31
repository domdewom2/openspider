import { analysis } from "../../../ipc/analysis";
import type { AnalysisStartArgs } from "../../../ipc/gen/AnalysisStartArgs";
import { useSession } from "../../../state/session";

/** Publish feedback before waiting for IPC, including analyses shorter than one tick. */
export async function startAnalysis(args: AnalysisStartArgs = {}) {
  const before = useSession.getState();
  useSession.setState({ analysisStarting: true, analysisResult: null });
  try { return await analysis.start(args); }
  catch (error) {
    useSession.setState({ analysisStarting: false, analysisResult: before.analysisResult });
    throw error;
  }
}
