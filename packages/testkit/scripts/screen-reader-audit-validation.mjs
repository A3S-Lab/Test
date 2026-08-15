const AUDIT_PROTOCOL = "a3s.test.screen-reader-audit/1";
const WORKFLOW_PROTOCOL = "a3s.test.screen-reader-workflows/1";
const OUTCOMES = new Set(["passed", "failed", "blocked"]);
const MAX_RESULTS = 100;
const MAX_EVIDENCE_PER_WORKFLOW = 20;

function isObject(value) {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function boundedString(value, maximum = 256) {
  return (
    typeof value === "string" &&
    value.trim().length > 0 &&
    value.length <= maximum
  );
}

function unknownFields(errors, label, value, allowed) {
  if (!isObject(value)) return;
  const admitted = new Set(allowed);
  for (const field of Object.keys(value)) {
    if (!admitted.has(field)) {
      errors.push(`${label} contains unknown field ${field}.`);
    }
  }
}

function isIsoTimestamp(value) {
  if (typeof value !== "string" || value.length > 64) return false;
  const match =
    /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,9}))?(Z|([+-])(\d{2}):(\d{2}))$/.exec(
      value,
    );
  if (!match) return false;
  const [year, month, day, hour, minute, second] = match
    .slice(1, 7)
    .map(Number);
  const leapYear = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const daysInMonth = [
    31,
    leapYear ? 29 : 28,
    31,
    30,
    31,
    30,
    31,
    31,
    30,
    31,
    30,
    31,
  ];
  if (
    month < 1 ||
    month > 12 ||
    day < 1 ||
    day > daysInMonth[month - 1] ||
    hour > 23 ||
    minute > 59 ||
    second > 59
  ) {
    return false;
  }
  if (match[8] !== "Z" && (Number(match[10]) > 23 || Number(match[11]) > 59)) {
    return false;
  }
  return Number.isFinite(Date.parse(value));
}

function safeEvidencePath(value) {
  if (!boundedString(value, 512) || value.includes("\\")) return false;
  if (value.startsWith("/") || /^[A-Za-z]:\//.test(value)) return false;
  const segments = value.split("/");
  return segments.every(
    (segment) => segment.length > 0 && segment !== "." && segment !== "..",
  );
}

function stringList(value, maximumItems, maximumLength) {
  return (
    Array.isArray(value) &&
    value.length > 0 &&
    value.length <= maximumItems &&
    value.every((entry) => boundedString(entry, maximumLength))
  );
}

export function validateScreenReaderWorkflowManifest(manifest) {
  const errors = [];
  if (!isObject(manifest)) {
    return {
      errors: ["Workflow manifest must be a JSON object."],
      workflows: [],
    };
  }

  unknownFields(errors, "Workflow manifest", manifest, [
    "protocol",
    "workflows",
  ]);
  if (manifest.protocol !== WORKFLOW_PROTOCOL) {
    errors.push(`Workflow manifest protocol must be ${WORKFLOW_PROTOCOL}.`);
  }
  if (
    !Array.isArray(manifest.workflows) ||
    manifest.workflows.length === 0 ||
    manifest.workflows.length > MAX_RESULTS
  ) {
    errors.push("Workflow manifest must contain between 1 and 100 workflows.");
    return { errors, workflows: [] };
  }

  const ids = new Set();
  for (const [index, workflow] of manifest.workflows.entries()) {
    const label = `Workflow manifest entry ${index + 1}`;
    if (!isObject(workflow)) {
      errors.push(`${label} must be an object.`);
      continue;
    }
    unknownFields(errors, label, workflow, [
      "id",
      "title",
      "setup",
      "steps",
      "expected",
    ]);
    if (
      !boundedString(workflow.id, 96) ||
      !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(workflow.id)
    ) {
      errors.push(`${label}.id must be a bounded lowercase kebab-case string.`);
    } else if (ids.has(workflow.id)) {
      errors.push(`Workflow manifest id ${workflow.id} is duplicated.`);
    } else {
      ids.add(workflow.id);
    }
    if (!boundedString(workflow.title, 160)) {
      errors.push(`${label}.title must be a non-empty bounded string.`);
    }
    if (!boundedString(workflow.setup, 1_024)) {
      errors.push(`${label}.setup must be a non-empty bounded string.`);
    }
    if (!stringList(workflow.steps, 20, 1_024)) {
      errors.push(`${label}.steps must contain 1 to 20 bounded strings.`);
    }
    if (!stringList(workflow.expected, 20, 1_024)) {
      errors.push(`${label}.expected must contain 1 to 20 bounded strings.`);
    }
  }

  return { errors, workflows: manifest.workflows };
}

export function validateScreenReaderAudit({
  audit,
  expectedRevision,
  requirePass = false,
  testkitVersion,
  workflows,
}) {
  const errors = [];
  const summary = { blocked: 0, failed: 0, passed: 0, total: 0 };
  if (!isObject(audit)) {
    return { errors: ["Audit must be a JSON object."], summary };
  }

  unknownFields(errors, "Audit", audit, [
    "protocol",
    "revision",
    "testkit_version",
    "independent",
    "auditor",
    "environment",
    "started_at",
    "completed_at",
    "notes",
    "results",
  ]);

  if (audit.protocol !== AUDIT_PROTOCOL) {
    errors.push(`Audit protocol must be ${AUDIT_PROTOCOL}.`);
  }
  if (audit.revision !== expectedRevision) {
    errors.push(
      `Audit revision ${String(audit.revision)} does not match ${expectedRevision}.`,
    );
  }
  if (
    typeof audit.revision !== "string" ||
    !/^[0-9a-f]{40}$/.test(audit.revision)
  ) {
    errors.push("Audit revision must be a full lowercase Git commit SHA.");
  }
  if (audit.testkit_version !== testkitVersion) {
    errors.push(
      `Audit Test Kit version ${String(audit.testkit_version)} does not match ${testkitVersion}.`,
    );
  }
  if (audit.independent !== true) {
    errors.push("Audit must explicitly attest independent execution.");
  }

  if (!isObject(audit.auditor)) {
    errors.push("Audit auditor must be an object.");
  } else {
    unknownFields(errors, "Audit auditor", audit.auditor, [
      "id",
      "name",
      "organization",
    ]);
    if (!boundedString(audit.auditor.id, 128)) {
      errors.push("Audit auditor.id must be a non-empty bounded string.");
    }
    for (const field of ["name", "organization"]) {
      if (
        audit.auditor[field] !== undefined &&
        !boundedString(audit.auditor[field], 256)
      ) {
        errors.push(
          `Audit auditor.${field} must be a non-empty bounded string.`,
        );
      }
    }
  }

  if (!isObject(audit.environment)) {
    errors.push("Audit environment must be an object.");
  } else {
    unknownFields(errors, "Audit environment", audit.environment, [
      "os",
      "browser",
      "screen_reader",
      "input_modes",
      "locale",
      "hardware",
    ]);
    for (const field of ["os", "browser", "screen_reader"]) {
      if (!boundedString(audit.environment[field], 256)) {
        errors.push(
          `Audit environment.${field} must be a non-empty bounded string.`,
        );
      }
    }
    for (const field of ["locale", "hardware"]) {
      if (
        audit.environment[field] !== undefined &&
        !boundedString(audit.environment[field], 256)
      ) {
        errors.push(
          `Audit environment.${field} must be a non-empty bounded string.`,
        );
      }
    }
    const modes = audit.environment.input_modes;
    if (
      !Array.isArray(modes) ||
      modes.length === 0 ||
      modes.length > 8 ||
      modes.some((mode) => !boundedString(mode, 64))
    ) {
      errors.push(
        "Audit environment.input_modes must contain 1 to 8 bounded strings.",
      );
    } else if (new Set(modes).size !== modes.length) {
      errors.push(
        "Audit environment.input_modes must contain unique bounded strings.",
      );
    }
  }

  const validStartedAt = isIsoTimestamp(audit.started_at);
  const validCompletedAt = isIsoTimestamp(audit.completed_at);
  if (!validStartedAt) {
    errors.push("Audit started_at must be an ISO-8601 timestamp.");
  }
  if (!validCompletedAt) {
    errors.push("Audit completed_at must be an ISO-8601 timestamp.");
  }
  if (
    validStartedAt &&
    validCompletedAt &&
    Date.parse(audit.completed_at) < Date.parse(audit.started_at)
  ) {
    errors.push("Audit completed_at must not precede started_at.");
  }
  if (audit.notes !== undefined && !boundedString(audit.notes, 8_192)) {
    errors.push(
      "Audit notes must be a non-empty bounded string when provided.",
    );
  }

  const expectedIds = Array.isArray(workflows)
    ? workflows
        .map((workflow) => workflow?.id)
        .filter((id) => typeof id === "string")
    : [];
  const results = Array.isArray(audit.results) ? audit.results : [];
  summary.total = results.length;
  if (
    !Array.isArray(audit.results) ||
    audit.results.length > MAX_RESULTS ||
    audit.results.some((result) => !isObject(result))
  ) {
    errors.push("Audit results must be an array of at most 100 objects.");
  }

  const actualIds = results.map((result) => result?.workflow_id);
  if (JSON.stringify(actualIds) !== JSON.stringify(expectedIds)) {
    errors.push(
      "Audit results must cover every workflow exactly once in manifest order.",
    );
  }
  const counts = new Map();
  for (const id of actualIds) {
    if (typeof id === "string") counts.set(id, (counts.get(id) ?? 0) + 1);
  }
  for (const [id, count] of counts) {
    if (count > 1) errors.push(`Audit workflow ${id} is duplicated.`);
  }
  for (const id of expectedIds) {
    if (!counts.has(id)) errors.push(`Audit workflow ${id} is missing.`);
  }
  const expectedSet = new Set(expectedIds);
  for (const id of counts.keys()) {
    if (!expectedSet.has(id)) errors.push(`Audit workflow ${id} is unknown.`);
  }

  for (const [index, result] of results.entries()) {
    if (!isObject(result)) continue;
    const workflowId =
      typeof result.workflow_id === "string"
        ? result.workflow_id
        : `result ${index + 1}`;
    unknownFields(errors, `Workflow ${workflowId}`, result, [
      "workflow_id",
      "outcome",
      "notes",
      "evidence",
    ]);
    if (!boundedString(result.workflow_id, 96)) {
      errors.push(`Audit result ${index + 1} must name a bounded workflow_id.`);
    }
    if (!OUTCOMES.has(result.outcome)) {
      errors.push(
        `Workflow ${workflowId} outcome must be passed, failed, or blocked.`,
      );
    } else {
      summary[result.outcome] += 1;
    }
    if (
      ["failed", "blocked"].includes(result.outcome) &&
      !boundedString(result.notes, 8_192)
    ) {
      errors.push(
        `Workflow ${workflowId} requires notes for a ${result.outcome} outcome.`,
      );
    } else if (
      result.notes !== undefined &&
      !boundedString(result.notes, 8_192)
    ) {
      errors.push(
        `Workflow ${workflowId} notes must be a non-empty bounded string when provided.`,
      );
    }
    if (!Array.isArray(result.evidence) || result.evidence.length === 0) {
      errors.push(
        `Workflow ${workflowId} must reference at least one evidence artifact.`,
      );
    } else if (
      result.evidence.length > MAX_EVIDENCE_PER_WORKFLOW ||
      result.evidence.some((entry) => !safeEvidencePath(entry))
    ) {
      errors.push(
        `Workflow ${workflowId} evidence must contain at most 20 safe relative file paths.`,
      );
    } else if (new Set(result.evidence).size !== result.evidence.length) {
      errors.push(`Workflow ${workflowId} evidence paths must be unique.`);
    }
  }

  if (requirePass) {
    for (const result of results) {
      if (isObject(result) && result.outcome !== "passed") {
        errors.push(
          `Closure audit requires every workflow to pass; ${String(result.workflow_id)} is ${String(result.outcome)}.`,
        );
      }
    }
  }

  return { errors, summary };
}
