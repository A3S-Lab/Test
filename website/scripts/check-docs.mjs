import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defaultVersion, versions } from '../versions.mjs';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(scriptDirectory, '..');
const repositoryRoot = path.resolve(websiteRoot, '..');
const docsRoot = path.join(websiteRoot, 'docs');
const failures = [];

async function fileExists(filename) {
  try {
    await readFile(filename);
    return true;
  } catch {
    return false;
  }
}

async function listFiles(directory, root = directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const filename = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listFiles(filename, root)));
    } else if (entry.isFile()) {
      files.push(path.relative(root, filename));
    }
  }
  return files.sort();
}

const cargoToml = await readFile(
  path.join(repositoryRoot, 'Cargo.toml'),
  'utf8',
);
const workspaceVersion = cargoToml.match(
  /\[workspace\.package\][\s\S]*?\nversion\s*=\s*"([^"]+)"/,
)?.[1];
if (`v${workspaceVersion}` !== defaultVersion) {
  failures.push(
    `Default documentation ${defaultVersion} does not match workspace ${workspaceVersion}.`,
  );
}

const snapshots = JSON.parse(
  await readFile(path.join(websiteRoot, 'version-snapshots.json'), 'utf8'),
);
if (snapshots.current?.version !== defaultVersion) {
  failures.push(
    'version-snapshots.json does not identify the default version.',
  );
}
const archivedVersions = (snapshots.archives ?? []).map(
  (entry) => entry.version,
);
const expectedArchives = versions.filter(
  (version) => version !== defaultVersion,
);
if (JSON.stringify(archivedVersions) !== JSON.stringify(expectedArchives)) {
  failures.push('Archive versions do not match versions.mjs ordering.');
}

for (const version of versions) {
  const zhRoot = path.join(docsRoot, version, 'zh');
  const enRoot = path.join(docsRoot, version, 'en');
  const [zhFiles, enFiles] = await Promise.all([
    listFiles(zhRoot),
    listFiles(enRoot),
  ]);
  if (JSON.stringify(zhFiles) !== JSON.stringify(enFiles)) {
    failures.push(`${version} Chinese and English route trees differ.`);
  }

  for (const relative of zhFiles.filter((filename) =>
    filename.endsWith('.mdx'),
  )) {
    for (const locale of ['zh', 'en']) {
      const filename = path.join(docsRoot, version, locale, relative);
      const contents = await readFile(filename, 'utf8');
      if (!contents.startsWith('---\n') || !contents.includes('\ntitle: ')) {
        failures.push(
          `${path.relative(websiteRoot, filename)} lacks frontmatter title.`,
        );
      }
      if (contents.includes('—') || contents.includes('–')) {
        failures.push(
          `${path.relative(websiteRoot, filename)} contains a long dash.`,
        );
      }
    }
  }

  for (const locale of ['zh', 'en']) {
    const testKitGuide = await readFile(
      path.join(docsRoot, version, locale, 'guide', 'testkit.mdx'),
      'utf8',
    );
    const versionedPackageUrl =
      `https://github.com/A3S-Lab/Test/releases/download/${version}/` +
      'a3s-testkit.tgz';
    if (!testKitGuide.includes(versionedPackageUrl)) {
      failures.push(
        `${version} ${locale} Test Kit guide does not pin ${versionedPackageUrl}.`,
      );
    }
  }
}

for (const asset of ['a3s-logo.png', 'social-card.svg', 'social-card.png']) {
  if (!(await fileExists(path.join(docsRoot, 'public', asset)))) {
    failures.push(`Missing public asset ${asset}.`);
  }
}

for (const filename of [
  path.join(websiteRoot, 'theme', 'home-copy.ts'),
  path.join(websiteRoot, 'theme', 'components', 'HomeLayout.tsx'),
]) {
  const contents = await readFile(filename, 'utf8');
  if (contents.includes('—') || contents.includes('–')) {
    failures.push(
      `${path.relative(websiteRoot, filename)} contains a long dash.`,
    );
  }
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `Documentation parity verified for ${versions.length} versions and two locales.`,
);
