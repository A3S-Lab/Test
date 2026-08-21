export class BenchmarkSpecFailure extends Error {
  constructor(message) {
    super(message);
    this.name = "BenchmarkSpecFailure";
  }
}

export class CandidateBehaviorFailure extends Error {
  constructor(message) {
    super(message);
    this.name = "CandidateBehaviorFailure";
  }
}

export const TASKS = [
  task("click-button", "semantic-click", 1337, solveClickButton),
  task("enter-text", "form-fill", 7331, solveEnterText),
  task("choose-list", "single-select", 424242, solveChooseList),
  task("click-checkboxes", "multi-target-state", 17, solveCheckboxes),
  task("click-scroll-list", "multi-select-scroll", 23, solveScrollList),
  task("grid-coordinate", "geometry-targeting", 42, solveGridCoordinate),
  task("enter-text-dynamic", "dynamic-layout-form", 101, solveEnterText),
  task("click-menu-2", "dynamic-disclosure", 1337, solveMenu),
  task("click-dialog-2", "dialog-targeting", 202, solveDialog),
];

function task(id, dimension, seed, solve) {
  return { id, dimension, seed, solve };
}

async function solveClickButton(adapter, observation) {
  const label = capture(
    observation.snapshot,
    /Click on the \\"([^\"]+)\\" button\./,
    "click-button label",
  );
  const ref = findRef(observation, "button", label);
  await adapter.click(refTarget(ref), observation);
}

async function solveEnterText(adapter, observation) {
  const value = capture(
    observation.snapshot,
    /Enter \\"([^\"]+)\\" into the text field and press Submit\./,
    "text entry value",
  );
  await adapter.fill(cssTarget("#tt"), value);
  await adapter.click(cssTarget("#subbtn"));
}

async function solveChooseList(adapter, observation) {
  const value = capture(
    observation.snapshot,
    /Select (.+?) from the list and click Submit\./,
    "single-select value",
  );
  await adapter.select(cssTarget("#options"), [value]);
  await adapter.click(cssTarget("#area button"));
}

async function solveCheckboxes(adapter, observation) {
  const selection = capture(
    observation.snapshot,
    /Select (.+?) and click Submit\./,
    "checkbox labels",
  );
  if (selection !== "nothing") {
    for (const label of selection.split(", ")) {
      const current = adapter.currentObservation ?? observation;
      const ref = findRef(current, "checkbox", label);
      await adapter.check(refTarget(ref), current);
      await adapter.observe();
    }
  }
  await adapter.click(cssTarget("#subbtn"));
}

async function solveScrollList(adapter, observation) {
  const selection = capture(
    observation.snapshot,
    /Select (.+?) from the scroll list and click Submit\./,
    "multi-select values",
  );
  await adapter.select(cssTarget("#options"), selection.split(", "));
  await adapter.click(cssTarget("#area button"));
}

async function solveGridCoordinate(adapter, observation) {
  const coordinate = capture(
    observation.snapshot,
    /Click on the grid coordinate (\(-?\d,-?\d\))\./,
    "grid coordinate",
  );
  await adapter.click(cssTarget(`circle[id="${coordinate}"]`));
}

async function solveMenu(adapter, observation) {
  const labelMatch = observation.snapshot.match(
    /item labeled \\"([^\"]+)\\"\./,
  );
  if (!labelMatch) {
    throw new BenchmarkSpecFailure(
      "The locked click-menu-2 seed produced an icon-only goal.",
    );
  }
  const targetLabel = labelMatch[1];

  await adapter.click(cssTarget("#open-menu"));
  let menuObservation = await adapter.observe();
  let targetRef = findRefOrNull(menuObservation, "menuitem", targetLabel);

  if (!targetRef) {
    const playbackRef = findRef(menuObservation, "menuitem", "Playback");
    await adapter.click(refTarget(playbackRef), menuObservation);
    menuObservation = await adapter.observe();
    targetRef = findRefOrNull(menuObservation, "menuitem", targetLabel);
    if (!targetRef) {
      throw new CandidateBehaviorFailure(
        `The submenu did not expose menuitem ${JSON.stringify(targetLabel)}.`,
      );
    }
  }

  await adapter.click(refTarget(targetRef), menuObservation);
}

async function solveDialog(adapter, observation) {
  const label = capture(
    observation.snapshot,
    /dialog box labeled \\"([^\"]+)\\"\./,
    "dialog button label",
  );
  if (label === "x") {
    await adapter.click(cssTarget(".ui-dialog-titlebar-close"));
    return;
  }
  const ref = findRef(observation, "button", label);
  await adapter.click(refTarget(ref), observation);
}

export function findRef(observation, role, name) {
  const ref = findRefOrNull(observation, role, name);
  if (!ref) {
    throw new BenchmarkSpecFailure(
      `Observation has no exact ${role} ref named ${JSON.stringify(name)}.`,
    );
  }
  return ref;
}

function findRefOrNull(observation, role, name) {
  for (const [ref, metadata] of Object.entries(observation.refs ?? {})) {
    if (metadata.role === role && metadata.name === name) {
      return ref;
    }
  }
  return null;
}

function capture(source, pattern, description) {
  const match = source.match(pattern);
  if (!match) {
    throw new BenchmarkSpecFailure(
      `Could not parse ${description} from observation.`,
    );
  }
  return match[1];
}

export function cssTarget(selector) {
  return { kind: "css", selector };
}

export function refTarget(value) {
  return { kind: "ref", value };
}
