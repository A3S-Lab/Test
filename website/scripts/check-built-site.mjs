import { access, readFile, readdir, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defaultVersion, versions } from '../versions.mjs';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(scriptDirectory, '..');
const docsRoot = path.join(websiteRoot, 'docs');
const outputRoot = path.join(websiteRoot, 'doc_build');
const base = process.env.DOCS_BASE ?? '/Test/';
const failures = [];

async function collectFiles(directory, predicate) {
  const entries = await readdir(directory, { withFileTypes: true });
  const output = [];
  for (const entry of entries) {
    const filename = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      output.push(...(await collectFiles(filename, predicate)));
    } else if (entry.isFile() && predicate(filename)) {
      output.push(filename);
    }
  }
  return output;
}

function versionPrefix(version) {
  return version === defaultVersion ? '' : `${version}/`;
}

function localePrefix(locale) {
  return locale === 'zh' ? '' : `${locale}/`;
}

function routeForMdx(filename) {
  return filename
    .replace(/\.mdx$/, '.html')
    .split(path.sep)
    .join('/');
}

const requiredFiles = new Set([
  '404.html',
  'a3s-test-mark.svg',
  'favicon.svg',
  'social-card.svg',
  'social-card.png',
]);

for (const version of versions) {
  for (const locale of ['zh', 'en']) {
    const sourceRoot = path.join(docsRoot, version, locale);
    const mdxFiles = await collectFiles(sourceRoot, (filename) =>
      filename.endsWith('.mdx'),
    );
    for (const filename of mdxFiles) {
      requiredFiles.add(
        `${versionPrefix(version)}${localePrefix(locale)}${routeForMdx(
          path.relative(sourceRoot, filename),
        )}`,
      );
    }
    requiredFiles.add(
      `${versionPrefix(version)}${localePrefix(locale)}llms.txt`,
    );
    requiredFiles.add(
      `${versionPrefix(version)}${localePrefix(locale)}llms-full.txt`,
    );
  }
}

for (const relative of requiredFiles) {
  try {
    await access(path.join(outputRoot, relative));
  } catch {
    failures.push(`missing built file ${relative}`);
  }
}

const htmlFiles = await collectFiles(outputRoot, (filename) =>
  filename.endsWith('.html'),
);
const javascriptFiles = await collectFiles(
  path.join(outputRoot, 'static', 'js'),
  (filename) => filename.endsWith('.js'),
);
const javascript = (
  await Promise.all(
    javascriptFiles.map((filename) => readFile(filename, 'utf8')),
  )
).join('\n');

const rootHtml = await readFile(path.join(outputRoot, 'index.html'), 'utf8');
const englishHtml = await readFile(
  path.join(outputRoot, 'en', 'index.html'),
  'utf8',
);
const rootMarkdown = await readFile(path.join(outputRoot, 'index.md'), 'utf8');

if (!rootHtml.includes('<html lang="zh">')) {
  failures.push('default homepage does not declare Chinese');
}
if (!englishHtml.includes('<html lang="en">')) {
  failures.push('English homepage does not declare English');
}
if (!rootHtml.includes('看懂界面，') || !rootHtml.includes('证明每次操作')) {
  failures.push('default homepage lacks Chinese product copy');
}
if (
  !englishHtml.includes('See interfaces.') ||
  !englishHtml.includes('Prove actions.')
) {
  failures.push('English homepage lacks English product copy');
}
if (!rootHtml.includes('install.sh') || !javascript.includes('install.ps1')) {
  failures.push('homepage lacks cross-platform install commands');
}
if (
  !rootMarkdown.includes('install.sh') ||
  !rootMarkdown.includes('install.ps1')
) {
  failures.push('homepage Markdown lacks cross-platform install commands');
}
if (
  !rootHtml.includes(`CLI + Agent Skill<!-- --> · <!-- -->${defaultVersion}`)
) {
  failures.push(
    'homepage install panel does not identify the selected version',
  );
}
if (
  !javascript.includes('在 GitHub 上查看 A3S Test') ||
  !javascript.includes('View A3S Test on GitHub')
) {
  failures.push('navigation lacks localized GitHub accessible names');
}
if (!rootHtml.includes(`${base}social-card.png`)) {
  failures.push('homepage lacks the raster Open Graph image');
}

for (const version of versions.filter((entry) => entry !== defaultVersion)) {
  for (const locale of ['zh', 'en']) {
    const archiveHomepage = path.join(
      outputRoot,
      version,
      localePrefix(locale),
      'index.html',
    );
    const html = await readFile(archiveHomepage, 'utf8');
    if (!html.includes(`--version ${version}`)) {
      failures.push(`${version}/${locale} homepage does not pin its installer`);
    }
    const markdown = await readFile(
      archiveHomepage.replace(/\.html$/, '.md'),
      'utf8',
    );
    if (
      !markdown.includes(`--version ${version}`) ||
      !markdown.includes(`-Version ${version}`)
    ) {
      failures.push(
        `${version}/${locale} homepage Markdown does not pin both installers`,
      );
    }
  }
}

async function resolvesToBuiltFile(relativeReference) {
  const decodedReference = decodeURIComponent(relativeReference);
  const candidates =
    decodedReference === '' || decodedReference.endsWith('/')
      ? [path.join(decodedReference, 'index.html')]
      : [
          decodedReference,
          `${decodedReference}.html`,
          path.join(decodedReference, 'index.html'),
        ];

  for (const candidate of candidates) {
    const outputPath = path.resolve(outputRoot, candidate);
    if (
      outputPath !== outputRoot &&
      !outputPath.startsWith(`${outputRoot}${path.sep}`)
    ) {
      continue;
    }

    try {
      if ((await stat(outputPath)).isFile()) return true;
    } catch {
      // Try the next supported output form.
    }
  }

  return false;
}

const referencePattern = /(?:href|src)="([^"]+)"/g;
for (const htmlFile of htmlFiles) {
  const html = await readFile(htmlFile, 'utf8');
  for (const [, rawReference] of html.matchAll(referencePattern)) {
    if (
      rawReference.startsWith('#') ||
      rawReference.startsWith('//') ||
      rawReference.startsWith('data:') ||
      rawReference.startsWith('mailto:') ||
      /^[a-z]+:\/\//i.test(rawReference)
    ) {
      continue;
    }

    if (rawReference.startsWith('/') && !rawReference.startsWith(base)) {
      failures.push(
        `${path.relative(outputRoot, htmlFile)} references ${rawReference} outside ${base}`,
      );
      continue;
    }
    if (!rawReference.startsWith(base)) continue;

    const withoutBase = rawReference
      .slice(base.length)
      .split(/[?#]/, 1)[0]
      .replace(/\/+/g, '/');
    if (!(await resolvesToBuiltFile(withoutBase))) {
      failures.push(
        `${path.relative(outputRoot, htmlFile)} has broken reference ${rawReference}`,
      );
    }
  }
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `Built site verified across ${htmlFiles.length} HTML routes, ${versions.length} versions, and two locales.`,
);
