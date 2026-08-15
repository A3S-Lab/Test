const DOCUMENTATION_VERSION_PATTERN =
  /^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const PACKAGE_VERSION_PATTERN =
  /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const COMMIT_PATTERN = /^[0-9a-f]{40}$/;

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function parseWorkspaceVersion(workspaceManifest) {
  const lines = workspaceManifest.split(/\r?\n/);
  const sectionStart = lines.findIndex(
    (line) => line.trim() === "[workspace.package]",
  );
  if (sectionStart < 0) {
    throw new Error("Cargo.toml has no workspace package version.");
  }

  for (const line of lines.slice(sectionStart + 1)) {
    if (line.trimStart().startsWith("[")) break;
    const version = line.match(/^version\s*=\s*"([^"]+)"\s*$/)?.[1];
    if (version) return version;
  }

  throw new Error("Cargo.toml has no workspace package version.");
}

export function parseTestKitVersion(testKitManifest) {
  let manifest;
  try {
    manifest = JSON.parse(testKitManifest);
  } catch {
    throw new Error("Test Kit package manifest is not valid JSON.");
  }
  if (manifest?.name !== "@a3s-lab/testkit") {
    throw new Error("Test Kit package manifest has an unexpected package name.");
  }
  if (!PACKAGE_VERSION_PATTERN.test(manifest.version ?? "")) {
    throw new Error("Test Kit package manifest has no semantic version.");
  }
  return manifest.version;
}

export function validateReleaseMetadata({
  changelog,
  defaultVersion,
  releaseTag,
  snapshots,
  testKitManifest,
  versions,
  workspaceManifest,
}) {
  const workspaceVersion = parseWorkspaceVersion(workspaceManifest);
  const testKitVersion = parseTestKitVersion(testKitManifest);
  const expectedTag = `v${workspaceVersion}`;
  const errors = [];

  if (releaseTag !== undefined && releaseTag !== expectedTag) {
    errors.push(
      `Release tag ${releaseTag} does not match workspace version ${expectedTag}.`,
    );
  }
  if (defaultVersion !== expectedTag) {
    errors.push(
      `Default documentation ${defaultVersion} does not match workspace version ${expectedTag}.`,
    );
  }
  if (versions[0] !== defaultVersion) {
    errors.push(
      `The first documentation version must be the default version ${defaultVersion}.`,
    );
  }
  if (new Set(versions).size !== versions.length) {
    errors.push("Documentation versions must be unique.");
  }
  for (const version of versions) {
    if (!DOCUMENTATION_VERSION_PATTERN.test(version)) {
      errors.push(
        `Documentation version ${version} is not a supported vMAJOR.MINOR.PATCH version.`,
      );
    }
  }

  const current = snapshots?.current ?? {};
  if (current.version !== expectedTag) {
    errors.push(
      `Current snapshot ${current.version ?? "<missing>"} does not match workspace version ${expectedTag}.`,
    );
  }
  if (current.sourceTag !== expectedTag) {
    errors.push(
      `Current snapshot source tag ${current.sourceTag ?? "<missing>"} does not match ${expectedTag}.`,
    );
  }
  if (!COMMIT_PATTERN.test(current.sourceCommit ?? "")) {
    errors.push(
      "Current snapshot source commit must be a full lowercase Git commit SHA.",
    );
  }
  const currentTestKitVersion = current.testkitVersion ?? "<missing>";
  if (!PACKAGE_VERSION_PATTERN.test(currentTestKitVersion)) {
    errors.push(
      `Current snapshot Test Kit version ${currentTestKitVersion} is not semantic.`,
    );
  } else if (currentTestKitVersion !== testKitVersion) {
    errors.push(
      `Current snapshot Test Kit version ${currentTestKitVersion} does not match package version ${testKitVersion}.`,
    );
  }

  const archives = snapshots?.archives ?? [];
  const archiveVersions = archives.map((archive) => archive.version);
  if (JSON.stringify(archiveVersions) !== JSON.stringify(versions.slice(1))) {
    errors.push(
      "Archive versions do not match the non-default documentation versions.",
    );
  }
  for (const archive of archives) {
    if (archive.sourceTag !== archive.version) {
      errors.push(
        `Archive ${archive.version} source tag ${archive.sourceTag ?? "<missing>"} does not match its version.`,
      );
    }
    if (!COMMIT_PATTERN.test(archive.sourceCommit ?? "")) {
      errors.push(
        `Archive ${archive.version} source commit must be a full lowercase Git commit SHA.`,
      );
    }
    const archiveTestKitVersion = archive.testkitVersion ?? "<missing>";
    if (!PACKAGE_VERSION_PATTERN.test(archiveTestKitVersion)) {
      errors.push(
        `Archive ${archive.version} Test Kit version ${archiveTestKitVersion} is not semantic.`,
      );
    }
  }

  const releaseHeading = new RegExp(
    `^## ${escapeRegExp(workspaceVersion)} - \\d{4}-\\d{2}-\\d{2}\\s*$`,
    "m",
  );
  if (!releaseHeading.test(changelog)) {
    errors.push(
      `CHANGELOG.md has no dated ${workspaceVersion} release section.`,
    );
  }

  return { errors, expectedTag, workspaceVersion };
}
