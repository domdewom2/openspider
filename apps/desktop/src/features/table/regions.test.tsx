import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { App, registerMainRegions } from "../../App";
import { createClient, setClient } from "../../ipc/client";
import { setLanguage } from "../../i18n";
import { tableFixture } from "./fixtures/core";
afterEach(() => { cleanup(); registerMainRegions({ tabs: undefined, table: undefined, detail: undefined }); setClient(null); });

// spec: UI-004
it("composes independent table and detail registrations and retains the scaffold defaults", async () => {
  setClient(createClient(tableFixture().core.transport())); await setLanguage("en");
  registerMainRegions({ table: () => <div data-testid="registered-table" /> });
  registerMainRegions({ detail: () => <div data-testid="registered-detail" /> });
  const mounted = render(<App />);
  expect(screen.getByTestId("registered-table")).toBeInTheDocument();
  expect(screen.getByTestId("registered-detail")).toBeInTheDocument();
  mounted.unmount(); registerMainRegions({ table: undefined, detail: undefined });
  render(<App />);
  expect(screen.queryByTestId("registered-table")).toBeNull();
  expect(screen.queryByTestId("registered-detail")).toBeNull();
  expect(document.querySelector('[data-empty-state="NO_DATA_YET"]')).not.toBeNull();
});
