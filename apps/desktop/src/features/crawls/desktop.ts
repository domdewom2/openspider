import { client } from "../../ipc/client";
import type { CrawlConfigDto } from "../../ipc/gen/CrawlConfigDto";
import type { OpHandle } from "../../ipc/gen/OpHandle";
export interface RepairResult { path: string; integrityOk: boolean; usable: boolean; lostRows: number }
export interface CrawlDesktop {
  chooseOpen(multiple: boolean, title: string): Promise<string[] | null>;
  chooseSave(title: string, name: string): Promise<string | null>;
  chooseDirectory(title: string): Promise<string | null>;
  check(path: string): Promise<void>;
  repair(path: string): Promise<OpHandle>;
  configuration(path: string): Promise<CrawlConfigDto>;
}
export const nativeCrawlDesktop: CrawlDesktop = {
  async chooseOpen(multiple, title) {
    const value = await client.invoke<string | string[] | null>("dialog.open", { options: { multiple, title, filters: [{ name: "OpenSpider", extensions: ["ospider"] }] } });
    return value === null ? null : Array.isArray(value) ? value : [value];
  },
  chooseSave: (title, name) => client.invoke("dialog.save", { options: { title, defaultPath: name, filters: [{ name: "OpenSpider", extensions: ["ospider"] }] } }),
  chooseDirectory: title => client.invoke("dialog.open", { options: { title, directory: true, multiple: false } }),
  check: path => client.invoke("crawl_file_check", { path }),
  repair: path => client.invoke("crawl_file_repair", { path }),
  configuration: path => client.invoke("crawl_file_config", { path }),
};
