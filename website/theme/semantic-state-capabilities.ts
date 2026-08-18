import type { Locale } from './home-copy-types';

type CapabilityItem = {
  signal: string;
  title: string;
  body: string;
};

export const semanticStateCapabilities: Record<Locale, CapabilityItem[]> = {
  zh: [
    {
      signal: 'EXPANDED · COLLAPSED',
      title: '展开与折叠状态',
      body: '优先读取 details.open，再读取合法 aria-expanded；缺失或未知证据不会被反向断言当成折叠。',
    },
    {
      signal: 'PRESSED · UNPRESSED',
      title: '按压开关状态',
      body: '只接受明确布尔 aria-pressed；mixed 保持为不支持，不会被误判为未按压。',
    },
    {
      signal: 'READONLY · WRITABLE',
      title: '只读与可写状态',
      body: '适用时以原生 readOnly 为准，并与 enabled 保持正交；真正可编辑必须同时证明 enabled 与 writable。',
    },
    {
      signal: 'REQUIRED · OPTIONAL',
      title: '必填与可选状态',
      body: '适用控件优先使用原生 required，其他控件只接受合法 aria-required；目标缺失不能证明 optional。',
    },
    {
      signal: 'INVALID · VALID',
      title: '无效与有效状态',
      body: '原生控件参与 Constraint Validation 时读取真实 validity，否则只接受定义内 aria-invalid；未知状态关闭失败。',
    },
  ],
  en: [
    {
      signal: 'EXPANDED · COLLAPSED',
      title: 'Expanded and collapsed state',
      body: 'Read details.open before valid aria-expanded. Missing or unknown evidence can never make the negative condition pass as collapsed.',
    },
    {
      signal: 'PRESSED · UNPRESSED',
      title: 'Pressed toggle state',
      body: 'Accept only explicit boolean aria-pressed. Mixed remains unsupported instead of being misclassified as unpressed.',
    },
    {
      signal: 'READONLY · WRITABLE',
      title: 'Read-only and writable state',
      body: 'Prefer native readOnly where applicable and keep it orthogonal to enabled. Editability requires both enabled and writable.',
    },
    {
      signal: 'REQUIRED · OPTIONAL',
      title: 'Required and optional state',
      body: 'Prefer native required on applicable controls and otherwise accept only valid aria-required. A missing target never proves optional.',
    },
    {
      signal: 'INVALID · VALID',
      title: 'Invalid and valid state',
      body: 'Read real Constraint Validation when a native control participates; otherwise accept only defined aria-invalid tokens and fail unknown state closed.',
    },
  ],
};
