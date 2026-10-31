// Package metadata uses the bytes and filenames produced by R01/R02/R04.
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync, writeFileSync, mkdirSync, lstatSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

export const schemaVersion = '1.9.0';
export function generate({ directory, output, version, repository, packageId, mode = 'all', baseUrl }) {
  if (!/^\d+\.\d+\.\d+(?:-(?:beta|rc)\.\d+)?$/.test(version)) throw Error('Ungültige Version');
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository)) throw Error('Ungültiges Repository');
  if (!['all', 'homebrew', 'winget'].includes(mode)) throw Error('Ungültiger Modus');
  // K3-08 remains open: no default package identifier may escape into publication.
  if (mode !== 'homebrew' && !/^[A-Za-z0-9]+(?:\.[A-Za-z0-9]+)+$/.test(packageId ?? '')) {
    throw Error('OPENSPIDER_PACKAGE_ID / --package-id fehlt (K3-08)');
  }
  const base = baseUrl ?? `https://github.com/${repository}/releases/download/v${version}`;
  const parsed = new URL(base);
  if (parsed.username || parsed.password || parsed.search || parsed.hash ||
      !(parsed.protocol === 'https:' || (parsed.protocol === 'http:' && parsed.hostname === '127.0.0.1'))) {
    throw Error('Nur HTTPS oder Loopback für CI; keine Zugangsdaten in URLs');
  }
  const assets = readdirSync(directory).filter(n => lstatSync(join(directory, n)).isFile());
  const pick = pattern => {
    const found = assets.filter(n => pattern.test(n));
    if (found.length !== 1) throw Error(`Erwartet genau ein Asset für ${pattern}: ${found.length}`);
    const name = found[0];
    return { name, url: `${parsed.href.replace(/\/$/, '')}/${encodeURIComponent(name)}`,
      hash: createHash('sha256').update(readFileSync(join(directory, name))).digest('hex') };
  };
  const result = {};
  if (mode !== 'winget') {
    // R01 copies the actual Tauri DMG name unchanged; universal is not an updater archive.
    const arm = pick(/^OpenSpider_[A-Za-z0-9.+-]+_aarch64\.dmg$/);
    const intel = pick(/^OpenSpider_[A-Za-z0-9.+-]+_(?:x64|x86_64)\.dmg$/);
    result['openspider.rb'] = `cask "openspider" do
  version "${version}"

  on_arm do
    sha256 "${arm.hash}"
    url "${arm.url}"
  end
  on_intel do
    sha256 "${intel.hash}"
    url "${intel.url}"
  end

  name "OpenSpider"
  desc "Website crawler and analysis desktop application"
  homepage "https://openspider.de/"

  auto_updates true
  depends_on macos: ">= :ventura"

  app "OpenSpider.app"
  binary "#{appdir}/OpenSpider.app/Contents/MacOS/openspider"

${readFileSync(new URL('./lifecycle.rb', import.meta.url), 'utf8')}
  uninstall launchctl: "de.openspider.task.*",
            delete: "~/Library/LaunchAgents/de.openspider.task.*.plist"

  zap trash: [
    "~/Library/Application Support/OpenSpider",
    "~/Library/Application Support/org.openspider.desktop",
    "~/Library/Caches/org.openspider.desktop",
    "~/Library/Caches/OpenSpider",
    "~/Library/Logs/OpenSpider",
    "~/Library/Preferences/org.openspider.desktop.plist",
  ]
end
`;
  }
  if (mode !== 'homebrew') {
    // The installer patterns match scripts/release/manifest.mjs, without guessing versions/locales.
    const msi = pick(/^OpenSpider_[A-Za-z0-9.+-]+_x64_[A-Za-z-]+\.msi$/);
    const nsis = pick(/^OpenSpider_[A-Za-z0-9.+-]+_x64-setup\.exe$/);
    const common = { PackageIdentifier: packageId, PackageVersion: version };
    const docs = {
      version: { ...common, DefaultLocale: 'en-US', ManifestType: 'version', ManifestVersion: schemaVersion },
      installer: { ...common, MinimumOSVersion: '10.0.17763.0', Installers: [
        { Architecture: 'x64', InstallerType: 'msi', Scope: 'machine', InstallerUrl: msi.url,
          InstallerSha256: msi.hash.toUpperCase(), InstallerSwitches: {
            Silent: '/qn /norestart', SilentWithProgress: '/qb /norestart', Custom: 'ALLUSERS=1 ADDTOPATH=1', InstallLocation: 'INSTALLDIR="<INSTALLPATH>"' } },
        { Architecture: 'x64', InstallerType: 'nullsoft', Scope: 'user', InstallerUrl: nsis.url,
          InstallerSha256: nsis.hash.toUpperCase(), InstallerSwitches: { Silent: '/S', SilentWithProgress: '/S', InstallLocation: '/D=<INSTALLPATH>' } },
      ], ManifestType: 'installer', ManifestVersion: schemaVersion },
      'locale.en-US': { ...common, PackageLocale: 'en-US', Publisher: 'OpenSpider',
        PackageName: 'OpenSpider', License: 'Apache-2.0', ShortDescription: 'Website crawler and analysis desktop application',
        PackageUrl: 'https://openspider.de/', ManifestType: 'defaultLocale', ManifestVersion: schemaVersion },
    };
    for (const [suffix, doc] of Object.entries(docs)) {
      const type = doc.ManifestType;
      // JSON scalar quoting is a subset of YAML; all documents remain ordinary YAML mappings.
      result[`${packageId}.${suffix === 'version' ? '' : `${suffix}.`}yaml`] =
        `# yaml-language-server: $schema=https://aka.ms/winget-manifest.${type}.${schemaVersion}.schema.json\n` + yaml(doc);
    }
  }
  mkdirSync(output, { recursive: true });
  // Fail closed on stale/mixed manifests; callers must use an empty output directory.
  if (readdirSync(output).length) throw Error('Ausgabeordner muss leer sein');
  for (const [name, content] of Object.entries(result)) writeFileSync(join(output, name), content);
  return result;
}

function yaml(value, indent = '') {
  return Object.entries(value).map(([key, item]) => {
    if (Array.isArray(item)) return `${indent}${key}:\n` + item.map(v => `${indent}  -\n${yaml(v, `${indent}    `)}`).join('');
    if (typeof item === 'object') return `${indent}${key}:\n${yaml(item, `${indent}  `)}`;
    return `${indent}${key}: ${JSON.stringify(item)}\n`;
  }).join('');
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const { values } = parseArgs({ options: Object.fromEntries(
    ['directory', 'output', 'version', 'repository', 'package-id', 'mode', 'base-url'].map(k => [k, { type: 'string' }])) });
  generate({ ...values, packageId: values['package-id'] ?? process.env.OPENSPIDER_PACKAGE_ID, baseUrl: values['base-url'] });
}
