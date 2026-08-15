#!/usr/bin/env node

import { createServer } from "node:http";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";
import { validateScreenReaderWorkflowManifest } from "./screen-reader-audit-validation.mjs";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const packageRoot = path.resolve(scriptDirectory, "..");
const HOST = "127.0.0.1";

function usage() {
  return [
    "Usage: node scripts/serve-screen-reader-audit.mjs [options]",
    "",
    "Options:",
    "  --port <port>  Loopback port, or 0 for an ephemeral port (default: 4173)",
    "  --json         Print a machine-readable readiness record",
    "  --help         Show this help",
  ].join("\n");
}

function parseArguments(argv) {
  let json = false;
  let port = 4173;
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--help") return { help: true };
    if (value === "--json") {
      json = true;
      continue;
    }
    if (value === "--port") {
      const encoded = argv[index + 1];
      if (encoded === undefined || !/^\d+$/.test(encoded)) {
        throw new Error("--port requires an integer between 0 and 65535");
      }
      port = Number(encoded);
      if (!Number.isSafeInteger(port) || port < 0 || port > 65_535) {
        throw new Error("--port requires an integer between 0 and 65535");
      }
      index += 1;
      continue;
    }
    throw new Error(`unknown option ${value}`);
  }
  return { help: false, json, port };
}

function responseBody(value) {
  return Buffer.isBuffer(value) ? value : Buffer.from(value, "utf8");
}

function send(response, method, status, contentType, value, extraHeaders = {}) {
  const body = responseBody(value);
  response.writeHead(status, {
    "Cache-Control": "no-store",
    Connection: "close",
    "Content-Length": body.length,
    "Content-Type": contentType,
    "Referrer-Policy": "no-referrer",
    "X-Content-Type-Options": "nosniff",
    ...extraHeaders,
  });
  if (method !== "HEAD") response.end(body);
  else response.end();
}

async function main() {
  let options;
  try {
    options = parseArguments(process.argv.slice(2));
  } catch (error) {
    console.error(`${error.message}\n\n${usage()}`);
    process.exitCode = 2;
    return;
  }
  if (options.help) {
    console.log(usage());
    return;
  }

  const workspace = await mkdtemp(
    path.join(tmpdir(), "a3s-test-screen-reader-audit-"),
  );
  let cleaned = false;
  const cleanup = async () => {
    if (cleaned) return;
    cleaned = true;
    await rm(workspace, { force: true, recursive: true });
  };
  process.once("exit", () => {
    if (!cleaned) rmSync(workspace, { force: true, recursive: true });
  });

  try {
    const bundlePath = path.join(workspace, "testkit.js");
    await build({
      bundle: true,
      entryPoints: [path.join(packageRoot, "src", "browser-fixture.tsx")],
      format: "esm",
      logLevel: "silent",
      outfile: bundlePath,
      platform: "browser",
      target: "es2022",
    });
    const [template, bundle, manifestBuffer] = await Promise.all([
      readFile(path.join(packageRoot, "src", "browser-fixture.html"), "utf8"),
      readFile(bundlePath),
      readFile(path.join(packageRoot, "screen-reader-audit", "workflows.json")),
    ]);
    const manifest = JSON.parse(manifestBuffer.toString("utf8"));
    const manifestValidation = validateScreenReaderWorkflowManifest(manifest);
    if (manifestValidation.errors.length > 0) {
      throw new Error(manifestValidation.errors.join("\n"));
    }
    const page = template
      .replaceAll("__INITIAL_REPAIRED__", "false")
      .replaceAll("__ACTION_LABEL__", "Broken action");
    const routes = new Map([
      ["/health", ["text/plain; charset=utf-8", "ready"]],
      ["/testkit.html", ["text/html; charset=utf-8", page]],
      ["/testkit.js", ["text/javascript; charset=utf-8", bundle]],
      [
        "/screen-reader-workflows.json",
        ["application/json; charset=utf-8", manifestBuffer],
      ],
    ]);

    const server = createServer((request, response) => {
      const method = request.method ?? "GET";
      if (method !== "GET" && method !== "HEAD") {
        send(
          response,
          method,
          405,
          "text/plain; charset=utf-8",
          "method not allowed",
          {
            Allow: "GET, HEAD",
          },
        );
        return;
      }
      let pathname;
      try {
        pathname = new URL(request.url ?? "/", `http://${HOST}`).pathname;
      } catch {
        send(response, method, 400, "text/plain; charset=utf-8", "bad request");
        return;
      }
      const route = routes.get(pathname);
      if (!route) {
        send(response, method, 404, "text/plain; charset=utf-8", "not found");
        return;
      }
      send(response, method, 200, route[0], route[1]);
    });

    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen({ host: HOST, port: options.port }, resolve);
    });
    const address = server.address();
    if (!address || typeof address === "string") {
      throw new Error("audit fixture did not expose a TCP address");
    }
    const url = `http://${HOST}:${address.port}/testkit.html`;
    if (options.json) {
      process.stdout.write(
        `${JSON.stringify({ protocol: "a3s.test.screen-reader-fixture/1", url })}\n`,
      );
    } else {
      process.stdout.write(`Screen-reader audit fixture: ${url}\n`);
    }

    let shuttingDown = false;
    const shutdown = async () => {
      if (shuttingDown) return;
      shuttingDown = true;
      server.closeIdleConnections?.();
      server.closeAllConnections?.();
      await new Promise((resolve, reject) => {
        server.close((error) => (error ? reject(error) : resolve()));
      });
      await cleanup();
    };
    for (const signal of ["SIGINT", "SIGTERM"]) {
      process.once(signal, () => {
        void shutdown().catch((error) => {
          console.error(error instanceof Error ? error.message : String(error));
          process.exitCode = 1;
        });
      });
    }
  } catch (error) {
    await cleanup();
    throw error;
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
