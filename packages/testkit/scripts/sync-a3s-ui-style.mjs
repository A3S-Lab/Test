import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const outputPath = fileURLToPath(new URL("../src/a3s-ui-style.generated.ts", import.meta.url));
const sources = [
  ["A3S_UI_FOUNDATION_SOURCE", "@a3s-lab/ui/styles/a3s-foundation.css"],
  ["A3S_UI_TASK_PANE_SOURCE", "@a3s-lab/ui/components/task-pane.css"],
  ["A3S_UI_TOOLBAR_SOURCE", "@a3s-lab/ui/components/toolbar.css"],
  ["A3S_UI_STATUS_BADGE_SOURCE", "@a3s-lab/ui/components/status-badge.css"],
];

const declarations = await Promise.all(sources.map(async ([name, request]) => {
  const css = await readFile(require.resolve(request), "utf8");
  return `export const ${name} = ${JSON.stringify(css)};`;
}));

await writeFile(outputPath, `// Generated from @a3s-lab/ui. Run npm run sync:a3s-ui after upgrading it.\n${declarations.join("\n")}\n`);
