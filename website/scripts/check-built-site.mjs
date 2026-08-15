import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(scriptDirectory, '..');
const outputRoot = path.join(websiteRoot, 'doc_build');

async function collectHtml(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const output = [];
  for (const entry of entries) {
    const filename = path.join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await collectHtml(filename)));
    if (entry.isFile() && entry.name.endsWith('.html')) output.push(filename);
  }
  return output;
}

async function collectFiles(directory, extension) {
  const entries = await readdir(directory, { withFileTypes: true });
  const output = [];
  for (const entry of entries) {
    const filename = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      output.push(...(await collectFiles(filename, extension)));
    }
    if (entry.isFile() && entry.name.endsWith(extension)) output.push(filename);
  }
  return output;
}

const required = [
  'index.html',
  'en/index.html',
  'guide/index.html',
  'en/guide/index.html',
  'v0.15.0/index.html',
  'v0.15.0/en/index.html',
  'v0.15.0/guide/index.html',
  'v0.15.0/en/guide/index.html',
  'a3s-test-mark.svg',
  'favicon.svg',
  'social-card.svg',
  'social-card.png',
];

const missing = [];
for (const relative of required) {
  try {
    await readFile(path.join(outputRoot, relative));
  } catch {
    missing.push(relative);
  }
}

const htmlFiles = await collectHtml(outputRoot);
if (htmlFiles.length < 28) {
  missing.push(`expected at least 28 HTML files, found ${htmlFiles.length}`);
}

const rootHtml = await readFile(path.join(outputRoot, 'index.html'), 'utf8');
const englishHtml = await readFile(
  path.join(outputRoot, 'en', 'index.html'),
  'utf8',
);
const javascriptFiles = await collectFiles(
  path.join(outputRoot, 'static', 'js'),
  '.js',
);
const javascript = (
  await Promise.all(
    javascriptFiles.map((filename) => readFile(filename, 'utf8')),
  )
).join('\n');
if (!rootHtml.includes('看懂界面，') || !rootHtml.includes('证明每次操作')) {
  missing.push('Chinese default homepage content');
}
if (
  !englishHtml.includes('See interfaces.') ||
  !englishHtml.includes('Prove actions.')
) {
  missing.push('English homepage content');
}
if (!rootHtml.includes('install.sh') || !javascript.includes('install.ps1')) {
  missing.push('cross-platform install commands on the homepage');
}
if (!rootHtml.includes('/Test/social-card.png')) {
  missing.push('raster Open Graph image metadata');
}

if (missing.length > 0) {
  for (const item of missing) console.error(`- Missing ${item}`);
  process.exit(1);
}

console.log(`Built site verified across ${htmlFiles.length} HTML routes.`);
