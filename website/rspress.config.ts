import * as path from 'node:path';
import { defineConfig } from '@rspress/core';
import { aclLanguage } from './acl-language';
import { remarkAclSyntax } from './remark-acl-syntax';
import { defaultVersion, versions } from './versions.mjs';

const base = process.env.DOCS_BASE ?? '/Test/';
const siteOrigin = process.env.DOCS_ORIGIN ?? 'https://a3s-lab.github.io';

export default defineConfig({
  root: path.join(__dirname, 'docs'),
  base,
  siteOrigin,
  title: 'A3S Test',
  description:
    'A3S Test gives coding agents fresh page observations, typed actions, inspectable evidence, and repeatable ACL regressions.',
  lang: 'zh',
  icon: '/a3s-logo.png',
  logo: '/a3s-logo.png',
  logoText: 'A3S Test',
  outDir: 'doc_build',
  llms: true,
  markdown: {
    remarkPlugins: [remarkAclSyntax],
    shiki: {
      langs: ['tsx', 'ts', 'js', aclLanguage],
    },
  },
  multiVersion: {
    default: defaultVersion,
    versions,
  },
  locales: [
    {
      lang: 'zh',
      label: '简体中文',
      title: 'A3S Test',
      description:
        'A3S Test 让编码 Agent 依据最新页面观察执行类型化动作，记录可复查证据，并把稳定路径固化为 ACL 回归。',
    },
    {
      lang: 'en',
      label: 'English',
      title: 'A3S Test',
      description:
        'A3S Test gives coding agents fresh page observations, typed actions, inspectable evidence, and repeatable ACL regressions.',
    },
  ],
  head: [
    ['meta', { name: 'theme-color', content: '#f5f7fb' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'A3S Test' }],
    [
      'meta',
      {
        property: 'og:image',
        content: `${siteOrigin}${base}social-card.png`,
      },
    ],
    ['meta', { property: 'og:image:width', content: '1200' }],
    ['meta', { property: 'og:image:height', content: '630' }],
    [
      'meta',
      {
        property: 'og:image:alt',
        content:
          'A3S Test connects fresh page observations, typed actions, human-authorized repair, and repeatable ACL regressions.',
      },
    ],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    [
      'meta',
      {
        name: 'twitter:image',
        content: `${siteOrigin}${base}social-card.png`,
      },
    ],
    (route) => [
      'link',
      {
        rel: 'canonical',
        href: `${siteOrigin}${base.replace(/\/$/, '')}${route.routePath}`,
      },
    ],
  ],
  themeConfig: {
    darkMode: false,
    search: true,
    localeRedirect: 'never',
    enableContentAnimation: false,
    editLink: {
      docRepoBaseUrl: 'https://github.com/A3S-Lab/Test/tree/main/website/docs',
    },
    lastUpdated: true,
    llmsUI: {
      placement: 'outline',
      viewOptions: ['markdownLink', 'chatgpt', 'claude'],
    },
    socialLinks: [
      {
        icon: 'github',
        mode: 'link',
        content: 'https://github.com/A3S-Lab/Test',
      },
    ],
  },
});
