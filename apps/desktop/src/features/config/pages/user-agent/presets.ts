// User-Agent-Voreinstellungen (CRAWL-114), Abbild von crates/os-fetch/resources/user-agents.toml.
// Ein vitest vergleicht diese Liste mit der Datei des Kerns; Änderungen dort erfordern die Nachführung hier.
export interface UaPreset {
  id: string;
  name: string;
  group: string;
  /** HTTP-User-Agent mit `{version}`; `null` = nur Token bzw. benutzerdefiniert. */
  userAgent: string | null;
  robotsToken: string;
}

export const UA_PRESETS: readonly UaPreset[] = [
  { id: "openspider", name: "OpenSpider", group: "OpenSpider", userAgent: "Mozilla/5.0 (compatible; OpenSpider/{version}; +https://openspider.de)", robotsToken: "OpenSpider" },
  { id: "googlebot-smartphone", name: "Googlebot Smartphone", group: "Suchmaschinen", userAgent: "Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Mobile Safari/537.36 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)", robotsToken: "Googlebot" },
  { id: "googlebot-desktop", name: "Googlebot Desktop", group: "Suchmaschinen", userAgent: "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)", robotsToken: "Googlebot" },
  { id: "googlebot-image", name: "Googlebot Image", group: "Suchmaschinen", userAgent: "Googlebot-Image/1.0", robotsToken: "Googlebot-Image" },
  { id: "googlebot-news", name: "Googlebot News", group: "Suchmaschinen", userAgent: "Googlebot-News", robotsToken: "Googlebot-News" },
  { id: "googlebot-video", name: "Googlebot Video", group: "Suchmaschinen", userAgent: "Googlebot-Video/1.0", robotsToken: "Googlebot-Video" },
  { id: "google-inspectiontool", name: "Google-InspectionTool", group: "Suchmaschinen", userAgent: "Mozilla/5.0 (compatible; Google-InspectionTool/1.0)", robotsToken: "Google-InspectionTool" },
  { id: "google-extended", name: "Google-Extended (nur Token)", group: "KI-Crawler", userAgent: null, robotsToken: "Google-Extended" },
  { id: "bingbot-mobile", name: "Bingbot Mobil", group: "Suchmaschinen", userAgent: "Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Mobile Safari/537.36 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)", robotsToken: "bingbot" },
  { id: "bingbot-desktop", name: "Bingbot Desktop", group: "Suchmaschinen", userAgent: "Mozilla/5.0 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)", robotsToken: "bingbot" },
  { id: "yandex", name: "Yandex", group: "Suchmaschinen", userAgent: "Mozilla/5.0 (compatible; YandexBot/3.0; +http://yandex.com/bots)", robotsToken: "Yandex" },
  { id: "baiduspider", name: "Baiduspider", group: "Suchmaschinen", userAgent: "Mozilla/5.0 (compatible; Baiduspider/2.0; +http://www.baidu.com/search/spider.html)", robotsToken: "Baiduspider" },
  { id: "duckduckbot", name: "DuckDuckBot", group: "Suchmaschinen", userAgent: "DuckDuckBot/1.1; (+http://duckduckgo.com/duckduckbot.html)", robotsToken: "DuckDuckBot" },
  { id: "applebot", name: "Applebot", group: "Suchmaschinen", userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/13.1.1 Safari/605.1.15 (Applebot/0.1; +http://www.apple.com/go/applebot)", robotsToken: "Applebot" },
  { id: "gptbot", name: "GPTBot", group: "KI-Crawler", userAgent: "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko); compatible; GPTBot/1.1; +https://openai.com/gptbot", robotsToken: "GPTBot" },
  { id: "claudebot", name: "ClaudeBot", group: "KI-Crawler", userAgent: "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; ClaudeBot/1.0; +claudebot@anthropic.com)", robotsToken: "ClaudeBot" },
  { id: "perplexitybot", name: "PerplexityBot", group: "KI-Crawler", userAgent: "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; PerplexityBot/1.0; +https://perplexity.ai/perplexitybot)", robotsToken: "PerplexityBot" },
  { id: "chrome-desktop", name: "Chrome Desktop", group: "Browser", userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36", robotsToken: "*" },
  { id: "chrome-mobile", name: "Chrome Mobil", group: "Browser", userAgent: "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Mobile Safari/537.36", robotsToken: "*" },
  { id: "safari-iphone", name: "Safari iPhone", group: "Browser", userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1", robotsToken: "*" },
  { id: "firefox", name: "Firefox", group: "Browser", userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:128.0) Gecko/20100101 Firefox/128.0", robotsToken: "*" },
  { id: "edge", name: "Edge", group: "Browser", userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36 Edg/138.0.0.0", robotsToken: "*" },
  { id: "custom", name: "Benutzerdefiniert", group: "Benutzerdefiniert", userAgent: null, robotsToken: "*" },
];
