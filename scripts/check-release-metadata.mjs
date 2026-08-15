import { readFile, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defaultVersion, versions } from "../website/versions.mjs";
import { validateReleaseMetadata } from "./release-metadata.mjs";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDirectory, "..");

function parseReleaseTag(arguments_) {
  if (arguments_.length === 0) return undefined;
  if (arguments_.length === 2 && arguments_[0] === "--tag") {
    return arguments_[1];
  }
  throw new Error(
    "usage: node scripts/check-release-metadata.mjs [--tag <tag>]",
  );
}

async function directoryExists(directory) {
  try {
    return (await stat(directory)).isDirectory();
  } catch {
    return false;
  }
}

async function main() {
  const releaseTag = parseReleaseTag(process.argv.slice(2));
  const [workspaceManifest, testKitManifest, changelog, snapshotsContents] =
    await Promise.all([
    readFile(path.join(repositoryRoot, "Cargo.toml"), "utf8"),
    readFile(
      path.join(repositoryRoot, "packages", "testkit", "package.json"),
      "utf8",
    ),
    readFile(path.join(repositoryRoot, "CHANGELOG.md"), "utf8"),
    readFile(
      path.join(repositoryRoot, "website", "version-snapshots.json"),
      "utf8",
    ),
    ]);
  const snapshots = JSON.parse(snapshotsContents);
  const result = validateReleaseMetadata({
    changelog,
    defaultVersion,
    releaseTag,
    snapshots,
    testKitManifest,
    versions,
    workspaceManifest,
  });

  for (const version of versions) {
    for (const locale of ["zh", "en"]) {
      const relative = path.join("website", "docs", version, locale);
      if (!(await directoryExists(path.join(repositoryRoot, relative)))) {
        result.errors.push(`Missing documentation directory ${relative}.`);
      }
    }
  }

  if (result.errors.length > 0) {
    for (const error of result.errors) console.error(`- ${error}`);
    process.exitCode = 1;
    return;
  }

  console.log(
    `Release metadata verified for ${result.expectedTag}, ${versions.length} documentation versions, and two locales.`,
  );
}

main().catch((error) => {
  console.error(`Release metadata check failed: ${error.message}`);
  process.exitCode = 1;
});
