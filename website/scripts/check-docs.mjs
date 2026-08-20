import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defaultVersion, publishedVersion, versions } from '../versions.mjs';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(scriptDirectory, '..');
const repositoryRoot = path.resolve(websiteRoot, '..');
const docsRoot = path.join(websiteRoot, 'docs');
const failures = [];

const [
  websiteReadme,
  websiteRegressionSuite,
  repositoryReadme,
  testKitReadme,
  testKitManifestContents,
] = await Promise.all([
  readFile(path.join(websiteRoot, 'README.md'), 'utf8'),
  readFile(
    path.join(repositoryRoot, 'tests', 'e2e', 'website-testkit.acl'),
    'utf8',
  ),
  readFile(path.join(repositoryRoot, 'README.md'), 'utf8'),
  readFile(
    path.join(repositoryRoot, 'packages', 'testkit', 'README.md'),
    'utf8',
  ),
  readFile(
    path.join(repositoryRoot, 'packages', 'testkit', 'package.json'),
    'utf8',
  ),
]);
const normalizedWebsiteReadme = websiteReadme.replace(/\s+/g, ' ');
const testKitManifest = JSON.parse(testKitManifestContents);
const currentRegistrySpec = `@a3s-lab/testkit@${testKitManifest.version}`;

const requiredRegressionActions = [
  ['screenshot', 'review-screenshot-evidence'],
  ['accessibility', 'review-evidence'],
  ['accessibility', 'semantic-evidence'],
  ['console', 'console-evidence'],
  ['page_errors', 'page-error-evidence'],
  ['screenshot', 'mobile-layout-screenshot-evidence'],
  ['accessibility', 'mobile-layout-evidence'],
  ['accessibility', 'mobile-semantic-evidence'],
  ['console', 'mobile-console-evidence'],
  ['page_errors', 'mobile-page-error-evidence'],
];

for (const [action, name] of requiredRegressionActions) {
  if (!websiteRegressionSuite.includes(`${action} "${name}" {`)) {
    failures.push(
      `Website regression suite lacks the ${action} action ${name}.`,
    );
  }
}

for (const claim of [
  'desktop and mobile viewport PNG evidence',
  'focused interactive accessibility trees',
  'complete semantic trees',
  'empty console and page-error evidence',
  'owned browser and preview-server cleanup',
  'macOS and Windows',
]) {
  if (!normalizedWebsiteReadme.includes(claim)) {
    failures.push(`Website README lacks the evidence claim: ${claim}.`);
  }
}

for (const [label, contents] of [
  ['repository README', repositoryReadme],
  ['Test Kit README', testKitReadme],
]) {
  for (const installDetail of [
    `npm install --save-dev ${currentRegistrySpec}`,
    'npm ls @a3s-lab/testkit',
    'npm Registry',
  ]) {
    if (!contents.includes(installDetail)) {
      failures.push(
        `${label} lacks Test Kit install detail: ${installDetail}.`,
      );
    }
  }
}

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
if (!versions.includes(publishedVersion)) {
  failures.push(
    `Published documentation ${publishedVersion} is not listed in versions.mjs.`,
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
  const installVersion =
    version === defaultVersion ? publishedVersion : version;
  const isStagedCurrent =
    version === defaultVersion && publishedVersion !== defaultVersion;
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
      const candidateMarker =
        locale === 'zh'
          ? ':::warning 发布候选'
          : ':::warning Release candidate';
      if (isStagedCurrent && !contents.includes(candidateMarker)) {
        failures.push(
          `${path.relative(websiteRoot, filename)} lacks its release-candidate disclosure.`,
        );
      }
      if (!isStagedCurrent && contents.includes(candidateMarker)) {
        failures.push(
          `${path.relative(websiteRoot, filename)} retains a stale release-candidate disclosure.`,
        );
      }
    }
  }

  for (const locale of ['zh', 'en']) {
    const quickStartGuide = await readFile(
      path.join(docsRoot, version, locale, 'guide', 'index.mdx'),
      'utf8',
    );
    if (
      version === defaultVersion &&
      (!quickStartGuide.includes(`--version ${installVersion}`) ||
        !quickStartGuide.includes(`-Version ${installVersion}`))
    ) {
      failures.push(
        `${version} ${locale} quick start does not pin published install version ${installVersion}.`,
      );
    }

    const installationGuide = await readFile(
      path.join(docsRoot, version, locale, 'guide', 'installation.mdx'),
      'utf8',
    );
    if (
      !installationGuide.includes(`--version ${installVersion}`) ||
      !installationGuide.includes(`-Version ${installVersion}`) ||
      !installationGuide.includes(`--tag ${installVersion}`)
    ) {
      failures.push(
        `${version} ${locale} installation guide does not pin published install version ${installVersion}.`,
      );
    }
    const versionedPackageUrl =
      `https://github.com/A3S-Lab/Test/releases/download/${installVersion}/` +
      'a3s-testkit.tgz';
    const snapshot =
      version === defaultVersion
        ? snapshots.current
        : snapshots.archives.find((entry) => entry.version === version);
    const registrySpec = `@a3s-lab/testkit@${snapshot?.testkitVersion}`;
    const registryInstall = `npm install --save-dev ${registrySpec}`;
    if (
      version === defaultVersion &&
      (!installationGuide.includes(registryInstall) ||
        !installationGuide.includes('./testkit.mdx'))
    ) {
      failures.push(
        `${version} ${locale} installation guide lacks the current Test Kit install path.`,
      );
    }
    if (version === defaultVersion) {
      for (const command of [
        `npm install --save-dev ${registrySpec}`,
        `pnpm add --save-dev ${registrySpec}`,
        `yarn add --dev ${registrySpec}`,
        `bun add --dev ${registrySpec}`,
      ]) {
        if (!installationGuide.includes(command)) {
          failures.push(
            `${version} ${locale} installation guide lacks package-manager command: ${command}.`,
          );
        }
      }
      if (installationGuide.includes('a3s-testkit.tgz')) {
        failures.push(
          `${version} ${locale} installation guide retains the obsolete Release tarball install.`,
        );
      }
    }
    const testKitGuide = await readFile(
      path.join(docsRoot, version, locale, 'guide', 'testkit.mdx'),
      'utf8',
    );
    if (
      version !== defaultVersion &&
      !testKitGuide.includes(versionedPackageUrl)
    ) {
      failures.push(
        `${version} ${locale} Test Kit guide does not pin ${versionedPackageUrl}.`,
      );
    }
    if (
      version === defaultVersion &&
      (!testKitGuide.includes(registryInstall) ||
        !testKitGuide.includes('npm ls @a3s-lab/testkit') ||
        !testKitGuide.includes('npm Registry') ||
        !testKitGuide.includes('GitHub OIDC provenance'))
    ) {
      failures.push(
        `${version} ${locale} Test Kit guide lacks the install, verification, or distribution explanation.`,
      );
    }
    if (
      version === defaultVersion &&
      [quickStartGuide, testKitGuide].some(
        (contents) =>
          !contents.includes('`@eN`') ||
          !contents.includes('`@cN`') ||
          !contents.includes('`@uN`'),
      )
    ) {
      failures.push(
        `${version} ${locale} current guides do not explain all public observation refs.`,
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
