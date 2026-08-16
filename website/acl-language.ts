import type { LanguageRegistration } from 'shiki';

export const aclLanguage = {
  name: 'acl',
  displayName: 'A3S ACL',
  scopeName: 'source.a3s-acl',
  repository: {},
  patterns: [
    {
      name: 'comment.line.double-slash.a3s-acl',
      match: '//.*$',
    },
    {
      name: 'comment.line.number-sign.a3s-acl',
      match: '#.*$',
    },
    {
      name: 'string.quoted.double.a3s-acl',
      begin: '"',
      end: '"',
      patterns: [
        {
          name: 'constant.character.escape.a3s-acl',
          match: '\\\\.',
        },
      ],
    },
    {
      name: 'keyword.declaration.a3s-acl',
      match: '\\b[A-Za-z_][A-Za-z0-9_-]*(?=\\s*(?:"[^"\\n]*"\\s*)*\\{)',
    },
    {
      name: 'variable.other.member.a3s-acl',
      match: '\\b[A-Za-z_][A-Za-z0-9_-]*(?=\\s*=)',
    },
    {
      name: 'constant.language.a3s-acl',
      match: '\\b(?:true|false|null)\\b',
    },
    {
      name: 'constant.numeric.a3s-acl',
      match: '\\b(?:0[xX][0-9a-fA-F]+|\\d+(?:\\.\\d+)?)\\b',
    },
    {
      name: 'keyword.operator.a3s-acl',
      match: '==|!=|>=|<=|=|>|<',
    },
    {
      name: 'punctuation.section.a3s-acl',
      match: '[{}\\[\\](),]',
    },
  ],
} satisfies LanguageRegistration;
