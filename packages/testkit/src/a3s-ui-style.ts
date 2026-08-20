import {
  A3S_UI_FOUNDATION_SOURCE,
  A3S_UI_STATUS_BADGE_SOURCE,
  A3S_UI_TASK_PANE_SOURCE,
  A3S_UI_TOOLBAR_SOURCE,
} from "./a3s-ui-style.generated";

function scopeFoundation(css: string): string {
  const darkMatch = css.match(/\.dark\s*\{([\s\S]*?)\}/);
  const scoped = css
    .replace(/:root(?=\s*\{)/, ".a3s-root")
    .replace(/\.dark(?=\s*\{)/, '.a3s-root[data-theme="dark"]');
  if (!darkMatch) return scoped;
  return `${scoped}\n@media (prefers-color-scheme: dark) {\n.a3s-root[data-theme="system"] {${darkMatch[1]}}\n}`;
}

export const A3S_UI_SHADOW_CSS = `
${scopeFoundation(A3S_UI_FOUNDATION_SOURCE)}
${A3S_UI_TASK_PANE_SOURCE}
${A3S_UI_TOOLBAR_SOURCE}
${A3S_UI_STATUS_BADGE_SOURCE}
`;
