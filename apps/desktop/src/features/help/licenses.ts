import notices from "../../../src-tauri/resources/third-party/THIRD_PARTY_NOTICES.txt?raw";
import notice from "../../../../../NOTICE?raw";

export const licenseDocuments = [
  { name: "NOTICE", text: notice },
  { name: "THIRD_PARTY_NOTICES.txt", text: notices },
];

/** Keep whole paragraphs so copyright lines and conditions retain their context. */
export function searchLicenseText(text: string, query: string): string {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return text;
  const matches = text.split(/\n\s*\n/).filter((paragraph) => paragraph.toLocaleLowerCase().includes(needle));
  const referenced = new Set(matches.flatMap((paragraph) => [...paragraph.matchAll(/License texts: ([^\n]+)/g)].flatMap((match) => match[1]!.match(/L\d+/g) ?? [])));
  // A package-name search includes its complete terms, even when the text is shared.
  const fullTexts = [...text.matchAll(/={80}\nLicense text (L\d+)\n\n([\s\S]*?)(?=={80}\nLicense text L\d+\n|$)/g)]
    .filter((match) => referenced.has(match[1]!)).map((match) => match[0].trim());
  return [...matches, ...fullTexts].join("\n\n");
}
