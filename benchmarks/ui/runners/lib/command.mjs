import { spawn } from "node:child_process";

export class CommandFailure extends Error {
  constructor(message, operation) {
    super(message);
    this.name = "CommandFailure";
    this.operation = operation;
  }
}

export function parseJsonOutput(operation) {
  const source = operation.stdout.trim();
  if (!source) {
    return null;
  }

  try {
    return JSON.parse(source);
  } catch (error) {
    throw new CommandFailure(
      `Command returned invalid JSON: ${error.message}`,
      operation,
    );
  }
}

export async function runCommand({
  executable,
  args,
  cwd,
  env = process.env,
  timeoutMs = 30_000,
  phase,
  requestBody = null,
}) {
  const startedAt = new Date().toISOString();
  const started = process.hrtime.bigint();
  const stdoutChunks = [];
  const stderrChunks = [];
  let timedOut = false;

  const result = await new Promise((resolve, reject) => {
    const child = spawn(executable, args, {
      cwd,
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    child.stdout.on("data", (chunk) => stdoutChunks.push(Buffer.from(chunk)));
    child.stderr.on("data", (chunk) => stderrChunks.push(Buffer.from(chunk)));
    child.on("error", reject);

    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
      setTimeout(() => child.kill("SIGKILL"), 1_000).unref();
    }, timeoutMs);

    child.on("close", (exitCode, signal) => {
      clearTimeout(timer);
      resolve({ exitCode, signal });
    });
  });

  const durationMs = Number(process.hrtime.bigint() - started) / 1_000_000;
  const stdout = Buffer.concat(stdoutChunks).toString("utf8");
  const stderr = Buffer.concat(stderrChunks).toString("utf8");
  const operation = {
    phase,
    executable,
    args,
    request_bytes: Buffer.byteLength(
      requestBody ?? JSON.stringify({ executable, args }),
    ),
    started_at: startedAt,
    duration_ms: round(durationMs),
    exit_code: result.exitCode,
    signal: result.signal,
    timed_out: timedOut,
    stdout_bytes: Buffer.byteLength(stdout),
    stderr_bytes: Buffer.byteLength(stderr),
    stdout,
    stderr,
  };

  return operation;
}

export function requireSuccess(operation, description) {
  if (operation.exit_code !== 0 || operation.timed_out) {
    throw new CommandFailure(
      `${description} failed with exit code ${operation.exit_code}`,
      operation,
    );
  }
  return operation;
}

function round(value) {
  return Math.round(value * 1_000) / 1_000;
}
