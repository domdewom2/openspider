import { expect, it } from "vitest";
import { safeClientPreview } from "./configPreview";
import { clientPreview, TEST_TOKEN } from "./fixtures/agent-m2-ui";

// spec: AGENT-043
it("marks only changed TOML lines and retains unchanged foreign configuration as context", () => {
  const preview = clientPreview("codex");
  preview.before = '[mcp_servers.foreign]\ncommand = "foreign-helper"\nargs = ["old"]\nenabled = true';
  preview.after = '[mcp_servers.foreign]\ncommand = "foreign-helper"\nargs = ["new"]\nenabled = true';
  expect(safeClientPreview(preview, "codex")).toEqual({
    after: '["mcp_servers"."foreign"]\n"command" = "foreign-helper"\n"args" = ["new"]\n"enabled" = true',
    diff: '--- before\n+++ after\n ["mcp_servers"."foreign"]\n "command" = "foreign-helper"\n-"args" = ["old"]\n+"args" = ["new"]\n "enabled" = true',
    canWrite: true,
  });
});

// spec: AGENT-043
it("compares redacted JSON lines while preserving context around additions and deletions", () => {
  const preview = clientPreview("claudeDesktop");
  preview.before = JSON.stringify({ keep: "foreign-helper", removed: 1, token: TEST_TOKEN, tail: true });
  preview.after = JSON.stringify({ keep: "foreign-helper", added: 2, token: `${TEST_TOKEN}-changed`, tail: true });
  preview.diff = `unsafe ${TEST_TOKEN}`;
  expect(safeClientPreview(preview, "claudeDesktop").diff).toBe([
    "--- before", "+++ after", " {", '   "keep": "foreign-helper",',
    '-  "removed": 1,', '+  "added": 2,', '   "token": "[redacted]",',
    '   "tail": true', " }",
  ].join("\n"));
});

// spec: AGENT-043
it("handles added, removed, repeated and unchanged lines without replacing the whole document", () => {
  const preview = clientPreview("codex");
  const document = '[a]\nvalue = 1\n[b]\nvalue = 1';
  preview.before = document;
  preview.after = '[a]\nvalue = 1\n[c]\nvalue = 2\n[b]\nvalue = 1';
  expect(safeClientPreview(preview, "codex").diff).toBe('--- before\n+++ after\n ["a"]\n "value" = 1\n+["c"]\n+"value" = 2\n ["b"]\n "value" = 1');
  preview.before = "";
  preview.after = "value = 1";
  expect(safeClientPreview(preview, "codex").diff).toBe('--- before\n+++ after\n+"value" = 1');
  preview.before = "value = 1";
  preview.after = "";
  expect(safeClientPreview(preview, "codex").diff).toBe('--- before\n+++ after\n-"value" = 1');
  preview.before = document;
  preview.after = `${document}\n# formatting-only change`;
  expect(safeClientPreview(preview, "codex")).toMatchObject({ diff: "", canWrite: false });
});

// spec: AGENT-043
it("does not offer confirmation when the only changes are hidden secrets", () => {
  const preview = clientPreview("claudeDesktop");
  preview.before = JSON.stringify({ token: TEST_TOKEN });
  preview.after = JSON.stringify({ token: `${TEST_TOKEN}-changed` });
  expect(safeClientPreview(preview, "claudeDesktop")).toEqual({ after: '{\n  "token": "[redacted]"\n}', diff: "", canWrite: false });
});
