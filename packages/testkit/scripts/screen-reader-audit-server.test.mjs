import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { readdir } from "node:fs/promises";
import { createInterface } from "node:readline";
import { tmpdir } from "node:os";
import test from "node:test";

async function auditTemporaryDirectories() {
  return (await readdir(tmpdir(), { withFileTypes: true }))
    .filter(
      (entry) =>
        entry.isDirectory() &&
        entry.name.startsWith("a3s-test-screen-reader-audit-"),
    )
    .map((entry) => entry.name)
    .sort();
}

test(
  "serves the shared audit fixture on an isolated loopback port",
  { timeout: 30_000 },
  async () => {
    const temporaryDirectoriesBefore = await auditTemporaryDirectories();
    const child = spawn(
      process.execPath,
      ["scripts/serve-screen-reader-audit.mjs", "--port", "0", "--json"],
      { cwd: process.cwd(), stdio: ["ignore", "pipe", "pipe"] },
    );
    const stderr = [];
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    const exit = new Promise((resolve) => {
      child.once("exit", (exitCode, signal) => resolve({ exitCode, signal }));
    });

    try {
      const lines = createInterface({ input: child.stdout });
      const line = await Promise.race([
        once(lines, "line").then(([value]) => value),
        exit.then(() => {
          throw new Error(
            `audit fixture exited before readiness: ${stderr.join("")}`,
          );
        }),
      ]);
      const ready = JSON.parse(line);
      assert.match(ready.url, /^http:\/\/127\.0\.0\.1:\d+\/testkit\.html$/);
      assert.equal(ready.protocol, "a3s.test.screen-reader-fixture/1");

      const [health, page, bundle, manifest, rejected, missing, head] =
        await Promise.all([
          fetch(new URL("/health", ready.url)),
          fetch(ready.url),
          fetch(new URL("/testkit.js", ready.url)),
          fetch(new URL("/screen-reader-workflows.json", ready.url)),
          fetch(ready.url, { method: "POST" }),
          fetch(new URL("/", ready.url)),
          fetch(ready.url, { method: "HEAD" }),
        ]);
      assert.equal(await health.text(), "ready");
      assert.match(await page.text(), /Screen-reader audit controls/);
      assert.ok((await bundle.arrayBuffer()).byteLength > 100_000);
      assert.equal(
        (await manifest.json()).protocol,
        "a3s.test.screen-reader-workflows/1",
      );
      assert.equal(rejected.status, 405);
      assert.equal(rejected.headers.get("allow"), "GET, HEAD");
      assert.equal(missing.status, 404);
      assert.equal(head.status, 200);
      assert.equal(await head.text(), "");
    } finally {
      if (child.exitCode === null && child.signalCode === null) {
        child.kill("SIGTERM");
      }
      const { exitCode, signal } = await exit;
      assert.equal(signal, null, stderr.join(""));
      assert.equal(exitCode, 0, stderr.join(""));
      assert.deepEqual(
        await auditTemporaryDirectories(),
        temporaryDirectoriesBefore,
      );
    }
  },
);
