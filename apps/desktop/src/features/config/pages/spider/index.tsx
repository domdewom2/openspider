// Spider (CRAWL-038 ff.): Crawl, Extraktion, Limits, Rendering, Erweitert und Voreinstellungen als Unterseiten der Gruppe „Spider“.
import { definePage, f } from "../../frame/definePage";
import type { ConfigPage, FieldDef } from "../../frame/types";
import { presetFields, Presets } from "./Presets";
import { Rendering, renderingFields } from "./Rendering";

const crawlFields: FieldDef[] = [
  f("scope.all_subdomains", { sectionKey: "config-sec-scope" }),
  f("scope.subfolder_only"),
  f("scope.crawl_outside_start_folder"),
  f("cdns", { kind: "lines", hintKey: "config-hint-cdns" }),
  f("spider.follow_internal_nofollow", { sectionKey: "config-sec-follow" }),
  f("spider.follow_external_nofollow"),
  f("spider.store_uncrawlable_links"),
  f("spider.internal", { kind: "storeCrawl", sectionKey: "config-sec-linktypes" }),
  f("spider.external", { kind: "storeCrawl" }),
  f("spider.canonicals", { kind: "storeCrawl" }),
  f("spider.pagination", { kind: "storeCrawl" }),
  f("spider.hreflang", { kind: "storeCrawl" }),
  f("spider.amp", { kind: "storeCrawl" }),
  f("spider.mobile_alternate", { kind: "storeCrawl" }),
  f("spider.meta_refresh", { kind: "storeCrawl" }),
  f("spider.images", { kind: "storeCrawl", sectionKey: "config-sec-resources" }),
  f("spider.css", { kind: "storeCrawl" }),
  f("spider.javascript", { kind: "storeCrawl" }),
  f("spider.media", { kind: "storeCrawl" }),
  f("spider.iframes", { kind: "storeCrawl" }),
];

const extractionFields: FieldDef[] = [
  f("extraction.page_titles", { sectionKey: "config-sec-onpage" }),
  f("extraction.meta_description"),
  f("extraction.meta_keywords"),
  f("extraction.h1"),
  f("extraction.h2"),
  f("extraction.indexability", { sectionKey: "config-sec-technical" }),
  f("extraction.meta_robots"),
  f("extraction.x_robots_tag"),
  f("extraction.response_time"),
  f("extraction.last_modified"),
  f("extraction.page_size"),
  f("extraction.hash"),
  f("extraction.word_count"),
  f("extraction.text_ratio"),
  f("extraction.readability"),
  f("extraction.forms"),
  f("extraction.http_headers", { sectionKey: "config-sec-store" }),
  f("extraction.cookies"),
  f("extraction.srcset"),
  f("extraction.store_html"),
  f("extraction.store_rendered_html", { hintKey: "config-hint-needs-js" }),
  f("extraction.accessibility", { hintKey: "config-hint-needs-js" }),
  f("extraction.structured_data.json_ld", { sectionKey: "config-sec-structured" }),
  f("extraction.structured_data.microdata"),
  f("extraction.structured_data.rdfa"),
  f("extraction.structured_data.case_sensitive"),
  f("extraction.structured_data.schema_org_validation"),
  f("extraction.structured_data.rich_results_validation"),
];

const limitsFields: FieldDef[] = [
  f("limits.max_urls"),
  f("limits.max_depth"),
  f("limits.max_folder_depth"),
  f("limits.max_redirects"),
  f("limits.max_page_kb", { unitKey: "config-unit-kb" }),
  f("limits.max_links_per_url"),
  f("limits.max_url_length", { unitKey: "config-unit-chars" }),
  f("limits.max_query_params"),
  f("limits.max_urls_per_subdomain"),
];

const advancedFields: FieldDef[] = [
  f("advanced.response_timeout_s", { sectionKey: "config-sec-fetch", unitKey: "config-unit-s" }),
  f("advanced.assume_html"),
  f("advanced.cookies", { kind: "radio" }),
  f("advanced.respect_noindex", { sectionKey: "config-sec-respect" }),
  f("advanced.respect_canonical"),
  f("advanced.respect_self_ref_meta_refresh"),
  f("advanced.always_follow_redirects"),
  f("advanced.always_follow_canonicals"),
  f("advanced.crawl_fragments"),
  f("advanced.html_validation", { sectionKey: "config-sec-checks" }),
  f("advanced.green_hosting"),
  f("http.version", { sectionKey: "config-sec-http" }),
  f("http.retries_5xx"),
  f("http.retries_network"),
  f("http.throttle_on_429"),
  f("http.respect_hsts"),
  f("http.ignore_certificate_errors", { hintKey: "config-hint-cert" }),
  f("network.resolve", { kind: "lines", sectionKey: "config-sec-network", hintKey: "config-hint-resolve" }),
];

export const pages: ConfigPage[] = [
  definePage({ id: "spider-crawl", group: "spider", order: 10, fields: crawlFields }),
  definePage({ id: "spider-extraction", group: "spider", order: 20, fields: extractionFields, anchors: ["extraction"] }),
  definePage({ id: "spider-limits", group: "spider", order: 30, fields: limitsFields, anchors: ["limits"] }),
  {
    ...definePage({ id: "spider-rendering", group: "spider", order: 40, fields: renderingFields, anchors: ["rendering"] }),
    Component: Rendering,
  },
  definePage({ id: "spider-advanced", group: "spider", order: 50, fields: advancedFields, anchors: ["advanced", "http", "network"] }),
  {
    ...definePage({ id: "spider-presets", group: "spider", order: 60, fields: presetFields, anchors: ["title", "meta_description", "url", "h1", "h2", "image", "content", "links", "validation", "duplicates", "issues"] }),
    Component: Presets,
  },
];
