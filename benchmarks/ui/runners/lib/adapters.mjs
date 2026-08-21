import { promises as fs } from "node:fs";
import { join } from "node:path";
import {
  CommandFailure,
  parseJsonOutput,
  requireSuccess,
  runCommand,
} from "./command.mjs";

const RESULT_SELECTOR = "#__a3s_ui_benchmark_result";

class AdapterBase {
  constructor({ cwd, session, commandTimeoutMs }) {
    this.cwd = cwd;
    this.session = session;
    this.commandTimeoutMs = commandTimeoutMs;
    this.operations = [];
    this.opened = false;
    this.currentObservation = null;
  }

  async record(command) {
    const operation = await runCommand({
      cwd: this.cwd,
      timeoutMs: this.commandTimeoutMs,
      ...command,
    });
    this.operations.push(operation);
    return operation;
  }

  async artifactInventory() {
    return { bytes: 0, files: [] };
  }
}

export class A3sTestAdapter extends AdapterBase {
  constructor(options) {
    super(options);
    this.executable = options.a3sTestExecutable;
    this.driverCommandTimeoutMs =
      options.driverCommandTimeoutMs ?? options.commandTimeoutMs;
    this.artifactsDir = null;
  }

  get id() {
    return "a3s-test";
  }

  async open(url, viewport) {
    const start = await this.record({
      executable: this.executable,
      args: [
        "agent",
        "start",
        url,
        "--session",
        this.session,
        "--goal",
        "Complete the deterministic MiniWoB task shown on the page.",
        "--success",
        "The official MiniWoB raw reward is positive.",
        "--browser-driver",
        "standalone",
        "--command-timeout-ms",
        String(this.driverCommandTimeoutMs),
        "--idle-timeout-ms",
        "300000",
        "--json",
      ],
      phase: "setup",
    });
    requireSuccess(start, "A3S Test session start");
    const startJson = parseJsonOutput(start);
    this.artifactsDir = startJson?.artifacts_dir ?? null;
    this.opened = true;

    const viewportOperation = await this.record({
      executable: this.executable,
      args: [
        "agent",
        "viewport",
        "--session",
        this.session,
        String(viewport.width),
        String(viewport.height),
        "--json",
      ],
      phase: "setup",
    });
    requireSuccess(viewportOperation, "A3S Test viewport");
  }

  async observe() {
    const operation = await this.record({
      executable: this.executable,
      args: ["agent", "observe", "--session", this.session, "--json"],
      phase: "observation",
    });
    requireSuccess(operation, "A3S Test observation");
    const body = parseJsonOutput(operation);
    const data = body?.output?.data?.data;
    if (!data || typeof data.snapshot !== "string") {
      throw new CommandFailure(
        "A3S Test observation has no snapshot.",
        operation,
      );
    }

    this.currentObservation = {
      id: body.observation_id,
      snapshot: data.snapshot,
      refs: data.refs ?? {},
    };
    return this.currentObservation;
  }

  async click(target, observation) {
    await this.action("click", { target }, observation);
  }

  async fill(target, value, observation) {
    await this.action("fill", { target, value }, observation);
  }

  async check(target, observation) {
    await this.action("check", { target }, observation);
  }

  async select(target, values, observation) {
    await this.action("select", { target, values }, observation);
  }

  async drag(source, target, observation) {
    await this.action("drag", { source, target }, observation);
  }

  async hover(target, observation) {
    await this.action("hover", { target }, observation);
  }

  async focus(target, observation) {
    await this.action("focus", { target }, observation);
  }

  async press(key) {
    await this.action("press", { key });
  }

  async wheel(deltaY) {
    await this.action("wheel", {
      delta_x: 0,
      delta_y: deltaY,
      modifiers: [],
    });
  }

  async action(type, fields, observation, phase = "action") {
    const action = { type, ...mapTargets(fields) };
    const observationId = referencedTargetKinds(fields).includes("ref")
      ? observation?.id
      : null;
    if (referencedTargetKinds(fields).includes("ref") && !observationId) {
      throw new Error(`A3S Test ${type} requires its source observation.`);
    }

    const args = [
      "agent",
      "act",
      "--session",
      this.session,
      "--action-json",
      JSON.stringify(action),
    ];
    if (observationId) {
      args.push("--observation", String(observationId));
    }
    args.push("--json");

    const operation = await this.record({
      executable: this.executable,
      args,
      phase,
      requestBody: JSON.stringify(action),
    });
    operation.action_type = type;
    operation.target_kinds = referencedTargetKinds(fields);
    requireSuccess(operation, `A3S Test ${type}`);
    return parseJsonOutput(operation);
  }

  async score() {
    const done = await this.scoreSelector(
      `${RESULT_SELECTOR}[data-benchmark-done="true"]`,
      true,
    );
    if (!done) {
      return { done: false, passed: false, raw_reward: null };
    }
    const passed = await this.scoreSelector(
      `${RESULT_SELECTOR}[data-benchmark-pass="true"]`,
      false,
    );
    return { done: true, passed, raw_reward: passed ? 1 : 0 };
  }

  async scoreSelector(selector, required) {
    const action = {
      type: "assert",
      expectation: {
        type: "visible_count",
        value: {
          target: { type: "css", selector },
          count: 1,
        },
      },
    };
    const operation = await this.record({
      executable: this.executable,
      args: [
        "agent",
        "act",
        "--session",
        this.session,
        "--action-json",
        JSON.stringify(action),
        "--json",
      ],
      phase: "score",
      requestBody: JSON.stringify(action),
    });
    operation.action_type = "assert";
    operation.target_kinds = ["css"];
    if (operation.exit_code === 0 && !operation.timed_out) {
      return true;
    }
    if (required) {
      throw new CommandFailure(
        "MiniWoB did not reach a terminal state.",
        operation,
      );
    }
    return false;
  }

  async finish(passed, summary) {
    if (!this.opened) {
      return;
    }
    const operation = await this.record({
      executable: this.executable,
      args: [
        "agent",
        "finish",
        "--session",
        this.session,
        "--status",
        passed ? "passed" : "failed",
        "--summary",
        summary,
        "--json",
      ],
      phase: "cleanup",
    });
    const expectedExitCode = passed ? 0 : 1;
    if (operation.exit_code !== expectedExitCode || operation.timed_out) {
      throw new CommandFailure("A3S Test finish failed.", operation);
    }
    const body = parseJsonOutput(operation);
    if (body?.cleanup_error) {
      throw new CommandFailure("A3S Test cleanup failed.", operation);
    }
    this.opened = false;
  }

  async abort() {
    if (!this.opened) {
      return;
    }
    const operation = await this.record({
      executable: this.executable,
      args: ["agent", "abort", "--session", this.session, "--json"],
      phase: "cleanup",
    });
    if (operation.exit_code !== 0) {
      throw new CommandFailure("A3S Test abort failed.", operation);
    }
    this.opened = false;
  }

  async artifactInventory() {
    if (!this.artifactsDir) {
      return { bytes: 0, files: [] };
    }
    const sessionDir = join(this.artifactsDir, "..");
    return inventory(sessionDir);
  }
}

export class AgentBrowserAdapter extends AdapterBase {
  constructor(options) {
    super(options);
    this.executable = options.agentBrowserExecutable;
  }

  get id() {
    return "agent-browser";
  }

  async open(url, viewport) {
    await this.rawCommand("open", [url], "setup");
    this.opened = true;
    await this.rawCommand(
      "set",
      ["viewport", String(viewport.width), String(viewport.height)],
      "setup",
    );
  }

  async observe() {
    const { operation, body } = await this.rawCommand(
      "snapshot",
      [],
      "observation",
    );
    const data = body?.data;
    if (!data || typeof data.snapshot !== "string") {
      throw new CommandFailure(
        "agent-browser observation has no snapshot.",
        operation,
      );
    }
    this.currentObservation = {
      id: null,
      snapshot: data.snapshot,
      refs: data.refs ?? {},
    };
    return this.currentObservation;
  }

  async click(target) {
    await this.rawAction("click", [rawTarget(target)], [target]);
  }

  async fill(target, value) {
    await this.rawAction("fill", [rawTarget(target), value], [target]);
  }

  async check(target) {
    await this.rawAction("check", [rawTarget(target)], [target]);
  }

  async select(target, values) {
    await this.rawAction("select", [rawTarget(target), ...values], [target]);
  }

  async drag(source, target) {
    await this.rawAction(
      "drag",
      [rawTarget(source), rawTarget(target)],
      [source, target],
    );
  }

  async hover(target) {
    await this.rawAction("hover", [rawTarget(target)], [target]);
  }

  async focus(target) {
    await this.rawAction("focus", [rawTarget(target)], [target]);
  }

  async press(key) {
    await this.rawAction("press", [key], []);
  }

  async wheel(deltaY) {
    const { operation, body } = await this.rawCommand(
      "mouse",
      ["wheel", String(deltaY), "0"],
      "action",
      JSON.stringify(["mouse", "wheel", deltaY, 0]),
    );
    operation.action_type = "wheel";
    operation.target_kinds = [];
    return body;
  }

  async rawAction(command, args, targets) {
    const { operation, body } = await this.rawCommand(
      command,
      args,
      "action",
      JSON.stringify([command, ...args]),
    );
    operation.action_type = command;
    operation.target_kinds = targets.map((target) => target.kind);
    return body;
  }

  async score() {
    const done = await this.attribute("data-benchmark-done");
    const passed = await this.attribute("data-benchmark-pass");
    const rawReward = Number(await this.attribute("data-benchmark-raw-reward"));
    return {
      done: done === "true",
      passed: passed === "true",
      raw_reward: Number.isFinite(rawReward) ? rawReward : null,
    };
  }

  async attribute(name) {
    const { operation, body } = await this.rawCommand(
      "get",
      ["attr", RESULT_SELECTOR, name],
      "score",
    );
    const value = body?.data?.value ?? body?.data;
    if (typeof value !== "string") {
      throw new CommandFailure(
        `agent-browser did not return ${name}.`,
        operation,
      );
    }
    return value;
  }

  async finish() {
    await this.close();
  }

  async abort() {
    await this.close();
  }

  async close() {
    if (!this.opened) {
      return;
    }
    await this.rawCommand("close", [], "cleanup");
    this.opened = false;
  }

  async rawCommand(command, args, phase, requestBody = null) {
    const operation = await this.record({
      executable: this.executable,
      args: ["--session", this.session, "--json", command, ...args],
      phase,
      requestBody,
    });
    requireSuccess(operation, `agent-browser ${command}`);
    const body = parseJsonOutput(operation);
    if (body?.success !== true) {
      throw new CommandFailure(`agent-browser ${command} failed.`, operation);
    }
    return { operation, body };
  }
}

function mapTargets(fields) {
  return Object.fromEntries(
    Object.entries(fields).map(([key, value]) => {
      if (isTarget(value)) {
        return [key, a3sTarget(value)];
      }
      return [key, value];
    }),
  );
}

function a3sTarget(target) {
  if (target.kind === "ref") {
    return { type: "ref", value: target.value };
  }
  if (target.kind === "css") {
    return { type: "css", selector: target.selector };
  }
  throw new Error(`Unsupported target kind: ${target.kind}`);
}

function rawTarget(target) {
  if (target.kind === "ref") {
    return `@${target.value.replace(/^@/, "")}`;
  }
  if (target.kind === "css") {
    return target.selector;
  }
  throw new Error(`Unsupported target kind: ${target.kind}`);
}

function referencedTargetKinds(fields) {
  return Object.values(fields)
    .filter(isTarget)
    .map((target) => target.kind);
}

function isTarget(value) {
  return (
    value !== null &&
    typeof value === "object" &&
    (value.kind === "ref" || value.kind === "css")
  );
}

async function inventory(root) {
  const files = [];
  let bytes = 0;

  async function visit(directory) {
    let entries;
    try {
      entries = await fs.readdir(directory, { withFileTypes: true });
    } catch (error) {
      if (error.code === "ENOENT") {
        return;
      }
      throw error;
    }

    for (const entry of entries) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        await visit(path);
      } else if (entry.isFile()) {
        const stat = await fs.stat(path);
        bytes += stat.size;
        files.push({ path: path.slice(root.length + 1), bytes: stat.size });
      }
    }
  }

  await visit(root);
  files.sort((left, right) => left.path.localeCompare(right.path));
  return { bytes, files };
}
