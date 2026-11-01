// GitHub artifact history, partitioned by OS/architecture/fixture size.
import { mkdirSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
const repository = process.env.GITHUB_REPOSITORY;
const token = process.env.GH_TOKEN;
if (!repository || !token)
  throw new Error("GITHUB_REPOSITORY and GH_TOKEN required");
const name = process.env.OS_PERF_ARTIFACT;
const headers = {
  Authorization: `Bearer ${token}`,
  Accept: "application/vnd.github+json",
  "X-GitHub-Api-Version": "2022-11-28",
};
const get = async (path) => {
  const response = await fetch(
    `https://api.github.com/repos/${repository}/${path}`,
    { headers },
  );
  if (!response.ok) throw new Error(`History API HTTP ${response.status}`);
  return response.json();
};
const { workflow_runs: runs } = await get(
  "actions/workflows/ui-nightly.yml/runs?per_page=100",
);
let count = 0;
for (const run of runs) {
  if (
    String(run.id) === process.env.GITHUB_RUN_ID ||
    run.head_branch !== process.env.GITHUB_REF_NAME ||
    run.status !== "completed"
  )
    continue;
  const { artifacts } = await get(`actions/runs/${run.id}/artifacts`);
  const artifact = artifacts.find((a) => a.name === name && !a.expired);
  if (!artifact) continue;
  const response = await fetch(artifact.archive_download_url, { headers });
  if (!response.ok) throw new Error(`Artifact HTTP ${response.status}`);
  const destination = `target/loadtest/history/${run.id}`;
  mkdirSync(destination, { recursive: true });
  const zip = `${destination}/history.zip`;
  writeFileSync(zip, Buffer.from(await response.arrayBuffer()));
  try {
    execFileSync(process.platform === "win32" ? "python" : "python3", [
      "-c",
      "import sys,zipfile; z=zipfile.ZipFile(sys.argv[1]); n='ui-report.json'; sys.exit(3) if n not in z.namelist() else None; open(sys.argv[2], 'wb').write(z.read(n))",
      zip, `${destination}/ui-report.json`,
    ]);
  } catch (error) {
    if (error.status === 3) { console.log(`History run ${run.id}: artifact has no completed UI report`); continue; }
    throw error;
  }
  if (++count === 7) break;
}
console.log(
  `History: ${count}/7 previous reports (${name}); missing history is explicitly marked no-baseline`,
);
