import { access, readFile, readdir, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defaultVersion, publishedVersion, versions } from '../versions.mjs';

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
  'a3s-logo.png',
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
const stylesheetFiles = await collectFiles(
  path.join(outputRoot, 'static', 'css'),
  (filename) => filename.endsWith('.css'),
);
const javascript = (
  await Promise.all(
    javascriptFiles.map((filename) => readFile(filename, 'utf8')),
  )
).join('\n');
const stylesheets = (
  await Promise.all(
    stylesheetFiles.map((filename) => readFile(filename, 'utf8')),
  )
).join('\n');

const rootHtml = await readFile(path.join(outputRoot, 'index.html'), 'utf8');
const englishHtml = await readFile(
  path.join(outputRoot, 'en', 'index.html'),
  'utf8',
);
const installationHtml = await readFile(
  path.join(outputRoot, 'guide', 'installation.html'),
  'utf8',
);
const testKitHtml = await readFile(
  path.join(outputRoot, 'guide', 'testkit.html'),
  'utf8',
);
const contractsHtml = await readFile(
  path.join(outputRoot, 'guide', 'contracts.html'),
  'utf8',
);
const quickStartHtml = await readFile(
  path.join(outputRoot, 'guide', 'index.html'),
  'utf8',
);
const englishQuickStartHtml = await readFile(
  path.join(outputRoot, 'en', 'guide', 'index.html'),
  'utf8',
);
const rootMarkdown = await readFile(path.join(outputRoot, 'index.md'), 'utf8');
const socialCardSvg = await readFile(
  path.join(outputRoot, 'social-card.svg'),
  'utf8',
);
const socialCardPng = await readFile(path.join(outputRoot, 'social-card.png'));

if (!rootHtml.includes('<html lang="zh">')) {
  failures.push('default homepage does not declare Chinese');
}
if (!englishHtml.includes('<html lang="en">')) {
  failures.push('English homepage does not declare English');
}
if (
  !rootHtml.includes('让 Agent 看清页面') ||
  !rootHtml.includes('把跑通的路径变成回归') ||
  !rootHtml.includes('一次完整测试，五个可检查步骤')
) {
  failures.push('default homepage lacks Chinese product copy');
}
if (
  !englishHtml.includes('Read rendered pages.') ||
  !englishHtml.includes('Preserve proven paths.') ||
  !englishHtml.includes('One complete test in five checkable steps')
) {
  failures.push('English homepage lacks English product copy');
}
if (
  !rootHtml.includes('PRD 和设计稿描述期望，不能替代浏览器可访问树') ||
  !englishHtml.includes(
    'PRDs and designs describe expectations; they do not replace the browser accessibility tree',
  ) ||
  !rootMarkdown.includes('PRD 和设计稿描述期望，不能替代浏览器可访问树')
) {
  failures.push('homepage lacks the source-to-contract authority boundary');
}
if (
  !rootHtml.includes('data-testid="a3s-experience-submit"') ||
  !rootHtml.includes('此演示只把问题保存在当前标签页') ||
  !rootHtml.includes('不连接修复 Agent')
) {
  failures.push('homepage lacks the local interactive Test Kit surface');
}
if (
  !quickStartHtml.includes('先选对入口') ||
  !quickStartHtml.includes('让页面提供组件、定位器和坐标') ||
  !quickStartHtml.includes('<code>@cN</code>') ||
  !englishQuickStartHtml.includes('Choose the right entry point') ||
  !englishQuickStartHtml.includes(
    'Expose components, locators, and geometry',
  ) ||
  !englishQuickStartHtml.includes('<code>@cN</code>')
) {
  failures.push('quick start lacks its task routes or observation-scoped refs');
}
if (
  !javascript.includes('a3s.test.page-context/1') ||
  !javascript.includes('data-a3s-testkit-overlay')
) {
  failures.push('built JavaScript does not include the real Test Kit runtime');
}
if (!rootHtml.includes('bf8ff2ac')) {
  failures.push('homepage build erased the approved direction contract');
}
if (
  !javascript.includes('data-a3s-previous-tabindex') ||
  !javascript.includes('rp-doc-layout__sidebar--open')
) {
  failures.push('built JavaScript lacks closed mobile sidebar isolation');
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
  !rootHtml.includes(`CLI + Agent Skill<!-- --> · <!-- -->${publishedVersion}`)
) {
  failures.push(
    'homepage install panel does not identify the published version',
  );
}
if (!rootHtml.includes(`--version ${publishedVersion}`)) {
  failures.push('homepage installer does not pin the published version');
}
if (
  !rootMarkdown.includes(`--version ${publishedVersion}`) ||
  !rootMarkdown.includes(`-Version ${publishedVersion}`)
) {
  failures.push(
    'homepage Markdown does not pin both published-version installers',
  );
}
if (
  defaultVersion !== publishedVersion &&
  (!rootHtml.includes('当前文档已进入下一版本准备阶段') ||
    !rootMarkdown.includes('当前文档已进入下一版本准备阶段'))
) {
  failures.push('staged homepage does not disclose its stable install version');
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
const pngSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
if (
  !socialCardSvg.includes('Read rendered pages.') ||
  !socialCardSvg.includes('Preserve proven paths.') ||
  !socialCardSvg.includes('href="a3s-logo.png"') ||
  socialCardPng.length < 24 ||
  !socialCardPng.subarray(0, pngSignature.length).equals(pngSignature) ||
  socialCardPng.readUInt32BE(16) !== 1200 ||
  socialCardPng.readUInt32BE(20) !== 630
) {
  failures.push('social card lacks current copy, logo, or 1200x630 PNG output');
}

const requiredLightCodeTheme = [
  '--rp-code-block-color:#2e3440',
  '--rp-code-block-bg:var(--rp-c-bg)',
  '--shiki-token-constant:#1976d2',
  '--shiki-token-string:#31a94d',
  '--shiki-token-keyword:#cf2727',
  '--shiki-token-function:#7041c8',
  '--shiki-token-string-expression:#218438',
];
for (const contract of requiredLightCodeTheme) {
  if (!stylesheets.includes(contract)) {
    failures.push(`documentation CSS lacks light code theme ${contract}`);
  }
}
if (
  stylesheets.includes('--rp-code-block-bg:#0d1b2f') ||
  stylesheets.includes('background:#0d1b2f!important')
) {
  failures.push('documentation CSS restores the obsolete forced dark code');
}
if (!stylesheets.includes('.rp-doc.rspress-doc{')) {
  failures.push('documentation typography is not scoped to the content root');
}
if (!installationHtml.includes('data-lang="powershell"')) {
  failures.push('installation guide lacks highlighted PowerShell code');
}
if (
  !testKitHtml.includes('data-lang="tsx"') ||
  !testKitHtml.includes('var(--shiki-token-constant)')
) {
  failures.push('Test Kit guide lacks highlighted TSX code');
}
if (
  !contractsHtml.includes('class="rp-codeblock language-acl"') ||
  !contractsHtml.includes('data-lang="acl"') ||
  !contractsHtml.includes('var(--shiki-token-keyword)') ||
  !contractsHtml.includes('var(--shiki-token-string-expression)')
) {
  failures.push('contract guide lacks highlighted ACL code');
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
