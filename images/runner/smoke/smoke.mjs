import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { access, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { constants } from "node:fs";
import process from "node:process";

const workspace = "/workspace";
const smokeRoot = "/opt/a3s-test-runner/smoke";
const browser = "/opt/a3s-test-runner/node_modules/.bin/agent-browser";
const port = 43117;
const maxOutputBytes = 4 * 1024 * 1024;

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

async function execute(label, args, timeoutMs = 60_000) {
  const child = spawn(args[0], args.slice(1), {
    cwd: workspace,
    detached: true,
    env: process.env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stdout = Buffer.alloc(0);
  let stderr = Buffer.alloc(0);
  let timedOut = false;
  let oversized = false;

  const append = (current, chunk) => {
    const next = Buffer.concat([current, chunk]);
    if (next.length > maxOutputBytes) {
      oversized = true;
      try {
        process.kill(-child.pid, "SIGKILL");
      } catch {
        // The bounded child may have exited between the event and the signal.
      }
      return next.subarray(0, maxOutputBytes);
    }
    return next;
  };
  child.stdout.on("data", (chunk) => {
    stdout = append(stdout, chunk);
  });
  child.stderr.on("data", (chunk) => {
    stderr = append(stderr, chunk);
  });

  const deadline = setTimeout(() => {
    timedOut = true;
    try {
      process.kill(-child.pid, "SIGKILL");
    } catch {
      // The bounded child may have exited at the deadline.
    }
  }, timeoutMs);
  let result;
  try {
    result = await new Promise((resolve, reject) => {
      child.once("error", reject);
      child.once("close", (code, signal) => resolve({ code, signal }));
    });
  } finally {
    clearTimeout(deadline);
  }

  if (timedOut || oversized || result.code !== 0) {
    throw new Error(
      `${label} failed (code=${result.code}, signal=${result.signal}, timedOut=${timedOut}, oversized=${oversized})\n${stderr.toString("utf8")}`,
    );
  }
  await writeFile(`${workspace}/${label}.stdout`, stdout);
  return stdout.toString("utf8");
}

async function executeJson(label, args, timeoutMs) {
  const output = await execute(label, args, timeoutMs);
  try {
    return JSON.parse(output);
  } catch (error) {
    throw new Error(`${label} returned invalid JSON: ${error.message}`);
  }
}

function assertEvidence(result, label) {
  const evidence = result.scenarios.flatMap((scenario) =>
    scenario.steps.flatMap((step) => step.output?.evidence ?? []),
  );
  assert(evidence.length > 0, `${label} did not retain evidence`);
  return evidence;
}

async function assertEvidenceFiles(evidence, label) {
  for (const item of evidence) {
    await access(item.path, constants.R_OK);
    const metadata = await stat(item.path);
    assert(metadata.isFile() && metadata.size > 0, `${label} evidence is empty: ${item.path}`);
  }
}

function fixtureServer() {
  return createServer((request, response) => {
    if (request.url !== "/") {
      response.writeHead(404, { "content-type": "text/plain; charset=utf-8" });
      response.end("not found");
      return;
    }
    response.writeHead(200, {
      "cache-control": "no-store",
      "content-type": "text/html; charset=utf-8",
      connection: "close",
    });
    response.end(
      "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Runner smoke</title></head><body><main><h1>Hermetic runner ready</h1><button type=\"button\">Run</button></main></body></html>",
    );
  });
}

async function listen(server) {
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", resolve);
  });
}

async function close(server) {
  await new Promise((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
}

async function assertPortReleased() {
  const probe = fixtureServer();
  await listen(probe);
  await close(probe);
}

async function assertNoOwnedProcesses() {
  const patterns = [
    "agent-browser",
    "chrome-headless-shell",
    "a3s-test-browser-watchdog",
    "a3s-test-tui-watchdog",
  ];
  const survivors = [];
  for (const entry of await readdir("/proc")) {
    if (!/^\d+$/.test(entry) || Number(entry) === process.pid) {
      continue;
    }
    let command;
    try {
      command = (await readFile(`/proc/${entry}/cmdline`, "utf8")).replaceAll("\0", " ");
    } catch {
      continue;
    }
    if (patterns.some((pattern) => command.includes(pattern))) {
      survivors.push(`${entry}: ${command}`);
    }
  }
  assert(survivors.length === 0, `owned processes survived:\n${survivors.join("\n")}`);

  const runtimeEntries = (await readdir("/tmp")).filter(
    (entry) => entry.startsWith("a3st-") || entry.startsWith("a3s-test-browser-"),
  );
  assert(runtimeEntries.length === 0, `private runtimes survived: ${runtimeEntries.join(", ")}`);
}

const inventory = await executeJson("worker-inventory", [
  "a3s-test",
  "worker",
  "inventory",
  "--browser-driver",
  "standalone",
  "--browser-executable",
  browser,
  "--max-parallel-scenarios",
  "1",
  "--compact",
]);
assert(inventory.protocol === "a3s.test.worker-capabilities/2", "worker protocol mismatch");
assert(inventory.runtime.operating_system === "linux", "runner must report Linux");
assert(inventory.runtime.architecture === "x86_64", "runner must report amd64");
assert(inventory.surfaces.length === 2, "runner must report exactly Web and TUI");
assert(inventory.surfaces[0].surface === "web", "Web inventory must be canonical first");
assert(inventory.surfaces[0].execution === "headless", "runner Web must be headless");
assert(inventory.surfaces[0].browser.version === "0.26.0", "browser version mismatch");
assert(
  !inventory.surfaces[0].browser.features.includes("exact_origin_containment"),
  "standalone browser must not claim exact-origin containment",
);
assert(inventory.surfaces[1].surface === "tui", "TUI inventory must be canonical second");
assert(inventory.surfaces.every((surface) => surface.surface !== "gui"), "GUI must not be advertised");

const schema = await executeJson("worker-schema", [
  "a3s-test",
  "worker",
  "schema",
  "--compact",
]);
assert(schema.authority === "scheduling_evidence", "worker schema authority mismatch");
assert(schema.invariants.authenticated === false, "inventory must remain self-reported");
assert(schema.invariants.authorizes_execution === false, "inventory must not authorize execution");
assert(
  schema.inventory_schema.properties.protocol.const === "a3s.test.worker-capabilities/2",
  "inventory schema must bind the protocol",
);
assert(
  schema.inventory_schema.properties.max_parallel_scenarios.minimum === 1 &&
    schema.inventory_schema.properties.max_parallel_scenarios.maximum === 64,
  "inventory schema must publish concurrency bounds",
);
assert(
  schema.invariants.external_image_identity_required === true,
  "image identity must remain external",
);

const remoteSchema = await executeJson("remote-worker-schema", [
  "a3s-test",
  "worker",
  "remote",
  "schema",
  "--compact",
]);
assert(remoteSchema.protocol === "a3s.test.remote-worker/3", "remote protocol mismatch");
assert(
  remoteSchema.invariants.transport_authentication_required === true,
  "remote worker must require transport authentication",
);
assert(
  remoteSchema.invariants.request_cannot_select_executables === true,
  "remote requests must not select executables",
);
assert(
  remoteSchema.invariants.request_cannot_select_applications === true,
  "remote requests must not select applications",
);
assert(
  remoteSchema.invariants.exact_host_permission_binding === true,
  "remote GUI jobs must bind exact host permissions",
);
assert(
  remoteSchema.invariants.transports_artifacts === false,
  "remote protocol must not claim artifact transport",
);

const license = await stat("/usr/share/licenses/a3s-test/LICENSE");
assert(license.isFile() && license.size > 0, "runner license must be present");

await executeJson("check-web", ["a3s-test", "check", `${smokeRoot}/web.acl`, "--json"]);
await executeJson("check-tui", ["a3s-test", "check", `${smokeRoot}/tui.acl`, "--json"]);

const server = fixtureServer();
await listen(server);
let webResult;
try {
  webResult = await executeJson(
    "web-result",
    [
      "a3s-test",
      "run",
      `${smokeRoot}/web.acl`,
      "--browser-driver",
      "standalone",
      "--browser-executable",
      browser,
      "--command-timeout-ms",
      "30000",
      "--idle-timeout-ms",
      "15000",
      "--cleanup-timeout-ms",
      "10000",
      "--infrastructure-retries",
      "0",
      "--json",
    ],
    60_000,
  );
} finally {
  await close(server);
}
assert(webResult.status === "passed", "Web smoke did not pass");
await assertEvidenceFiles(assertEvidence(webResult, "Web smoke"), "Web smoke");
await assertPortReleased();

const tuiResult = await executeJson("tui-result", [
  "a3s-test",
  "run",
  `${smokeRoot}/tui.acl`,
  "--tui-executable",
  `${smokeRoot}/tui-fixture.sh`,
  "--command-timeout-ms",
  "5000",
  "--cleanup-timeout-ms",
  "5000",
  "--infrastructure-retries",
  "0",
  "--json",
]);
assert(tuiResult.status === "passed", "TUI smoke did not pass");
await assertEvidenceFiles(assertEvidence(tuiResult, "TUI smoke"), "TUI smoke");

await assertNoOwnedProcesses();
process.stdout.write(
  `${JSON.stringify({ status: "passed", surfaces: ["web", "tui"], network: "loopback_only" })}\n`,
);
