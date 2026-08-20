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
const repairsHtml = await readFile(
  path.join(outputRoot, 'guide', 'repairs.html'),
  'utf8',
);
const englishRepairsHtml = await readFile(
  path.join(outputRoot, 'en', 'guide', 'repairs.html'),
  'utf8',
);
const pageContextHtml = await readFile(
  path.join(outputRoot, 'concepts', 'page-context.html'),
  'utf8',
);
const englishPageContextHtml = await readFile(
  path.join(outputRoot, 'en', 'concepts', 'page-context.html'),
  'utf8',
);
const authorityHtml = await readFile(
  path.join(outputRoot, 'concepts', 'authority-and-safety.html'),
  'utf8',
);
const englishAuthorityHtml = await readFile(
  path.join(outputRoot, 'en', 'concepts', 'authority-and-safety.html'),
  'utf8',
);
const capabilitiesHtml = await readFile(
  path.join(outputRoot, 'reference', 'capabilities.html'),
  'utf8',
);
const englishCapabilitiesHtml = await readFile(
  path.join(outputRoot, 'en', 'reference', 'capabilities.html'),
  'utf8',
);
const troubleshootingHtml = await readFile(
  path.join(outputRoot, 'guide', 'troubleshooting.html'),
  'utf8',
);
const englishTroubleshootingHtml = await readFile(
  path.join(outputRoot, 'en', 'guide', 'troubleshooting.html'),
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
  !rootHtml.includes('让 Agent 看懂真实界面') ||
  !rootHtml.includes('找到源码，留下回归') ||
  !rootHtml.includes('A3S Test 只解决三个关键问题') ||
  !rootHtml.includes('四层技术实现支撑这三个结果') ||
  !rootHtml.includes('data-testid="capability-group-context"') ||
  !rootHtml.includes('data-testid="capability-group-core"') ||
  !rootHtml.includes('data-testid="capability-group-execution"') ||
  !rootHtml.includes('data-testid="capability-group-evidence"') ||
  !rootHtml.includes('查看完整能力的入口、输出、证据和失败边界') ||
  !rootHtml.includes('组合浏览器语义与页面上下文') ||
  !rootHtml.includes('显式声明源码归属') ||
  !rootHtml.includes('四种权限互不冒充') ||
  !rootHtml.includes('在一个右侧界面完成标记') ||
  !rootHtml.includes('ACL 固化已证明路径')
) {
  failures.push('default homepage lacks Chinese product copy');
}
if (
  !englishHtml.includes('Understand the real interface.') ||
  !englishHtml.includes('Find the source. Keep the proof.') ||
  !englishHtml.includes('A3S Test solves three essential problems') ||
  !englishHtml.includes('Four implementation layers produce those results') ||
  !englishHtml.includes(
    'Inspect every capability entry point, output, evidence rule, and failure boundary',
  ) ||
  !englishHtml.includes('Combine browser semantics and page context') ||
  !englishHtml.includes('Declare source ownership explicitly') ||
  !englishHtml.includes('Keep four authorities independent') ||
  !englishHtml.includes('Keep marking in one right-side surface') ||
  !englishHtml.includes('Preserve proven paths as ACL')
) {
  failures.push('English homepage lacks English product copy');
}
if (
  !rootMarkdown.includes('四层技术实现支撑这三个结果') ||
  !rootMarkdown.includes('用页面修订绑定节点身份') ||
  !rootMarkdown.includes('显式声明源码归属') ||
  !rootMarkdown.includes('类型化控制层') ||
  !rootMarkdown.includes('驱动只实现能够证明的能力') ||
  !rootMarkdown.includes('用更新后的页面验证修改') ||
  !rootMarkdown.includes('ACL 固化已证明路径')
) {
  failures.push('homepage Markdown lacks the core implementation ledger');
}
if (
  !rootHtml.includes('高效 vibe coding 依赖一条可信反馈闭环') ||
  !englishHtml.includes(
    'Fast vibe coding depends on a trustworthy feedback loop',
  ) ||
  !rootMarkdown.includes('高效 vibe coding 依赖一条可信反馈闭环')
) {
  failures.push('homepage lacks the first-principles product thesis');
}
if (
  !rootHtml.includes('data-testid="a3s-experience-submit"') ||
  !rootHtml.includes('此演示只把问题保存在当前标签页') ||
  !rootHtml.includes('不连接修复 Agent')
) {
  failures.push('homepage lacks the local interactive Test Kit surface');
}
if (
  !quickStartHtml.includes('只选一个入口') ||
  !quickStartHtml.includes('Test Kit 在哪里接入') ||
  !quickStartHtml.includes('<code>@cN</code>') ||
  !quickStartHtml.includes('<code>@uN</code>') ||
  !englishQuickStartHtml.includes('Choose one entry point') ||
  !englishQuickStartHtml.includes('Where Test Kit enters') ||
  !englishQuickStartHtml.includes('<code>@cN</code>') ||
  !englishQuickStartHtml.includes('<code>@uN</code>')
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
  !socialCardSvg.includes('Observe the page.') ||
  !socialCardSvg.includes('Prove every action.') ||
  !socialCardSvg.includes('@c12 · button') ||
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
  !testKitHtml.includes('var(--shiki-token-constant)') ||
  !testKitHtml.includes('框架无关接入') ||
  !testKitHtml.includes('验证接入结果')
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
if (
  !pageContextHtml.includes('三类浏览器事实如何配合') ||
  !pageContextHtml.includes('预算、截断与分页') ||
  !pageContextHtml.includes('常见问题定位') ||
  !englishPageContextHtml.includes(
    'How three browser fact sources work together',
  ) ||
  !englishPageContextHtml.includes('Budgets, truncation, and pagination') ||
  !englishPageContextHtml.includes('Troubleshooting')
) {
  failures.push('Page Context guides lack field-level bilingual detail');
}
if (
  !repairsHtml.includes('草稿、保存和发送不是同一件事') ||
  !repairsHtml.includes('修复状态机') ||
  !repairsHtml.includes('A3S Test 如何验证修复') ||
  !englishRepairsHtml.includes(
    'Draft, save, and send are separate operations',
  ) ||
  !englishRepairsHtml.includes('Repair state machine') ||
  !englishRepairsHtml.includes('How A3S Test verifies a repair')
) {
  failures.push('repair guides lack the bilingual queue and verification flow');
}
if (
  !authorityHtml.includes('Origin 和网络是两道门') ||
  !authorityHtml.includes('模型 provider 只有建议权限') ||
  !authorityHtml.includes('部署检查') ||
  !englishAuthorityHtml.includes('Origin and network are separate gates') ||
  !englishAuthorityHtml.includes(
    'Model providers have advisory authority only',
  ) ||
  !englishAuthorityHtml.includes('Deployment checklist')
) {
  failures.push('authority guides lack bilingual operational boundaries');
}
if (
  !capabilitiesHtml.includes('核心会话') ||
  !capabilitiesHtml.includes('界面执行') ||
  !capabilitiesHtml.includes('实时语义状态断言') ||
  !capabilitiesHtml.includes('<code>network_route</code>') ||
  !capabilitiesHtml.includes('人工评审与修复') ||
  !capabilitiesHtml.includes('证据、回归与调度') ||
  !englishCapabilitiesHtml.includes('Core sessions') ||
  !englishCapabilitiesHtml.includes('Surface execution') ||
  !englishCapabilitiesHtml.includes('Live semantic-state assertions') ||
  !englishCapabilitiesHtml.includes('<code>network_route</code>') ||
  !englishCapabilitiesHtml.includes('Human review and repair') ||
  !englishCapabilitiesHtml.includes('Evidence, regression, and scheduling')
) {
  failures.push(
    'capability reference lacks bilingual entry and boundary detail',
  );
}
if (
  !troubleshootingHtml.includes('根据错误范围选择负责人') ||
  !troubleshootingHtml.includes('<code>test.driver.web.*</code>') ||
  !troubleshootingHtml.includes('Page Context 被截断') ||
  !troubleshootingHtml.includes('提交最小诊断包') ||
  !englishTroubleshootingHtml.includes('Assign ownership by error scope') ||
  !englishTroubleshootingHtml.includes('<code>test.driver.web.*</code>') ||
  !englishTroubleshootingHtml.includes('Page Context is truncated') ||
  !englishTroubleshootingHtml.includes('Share a minimal diagnostic bundle')
) {
  failures.push('troubleshooting guides lack bilingual evidence-led diagnosis');
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
