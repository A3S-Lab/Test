# A3S Test website

The official multilingual website and versioned documentation for
[A3S Test](https://github.com/A3S-Lab/Test), built with Rspress.

The root route is Simplified Chinese. English uses `/en/`, historical
versions keep the same locale tree under `/<version>/`, and the homepage
selects a desktop install command while keeping every command copyable.

## Local development

```bash
npm ci
npm run dev
```

The production site is served from `/Test/`. Override `DOCS_BASE` and
`DOCS_ORIGIN` only for another deployment target.

## Checks

```bash
npm run format:check
npm run check
npm run build
npm run check:site
```

The repository-level browser regression builds this production site, serves
it from an isolated loopback origin, and drives the real embedded Test Kit
experience through single and batch submissions. It also captures a
screenshot and accessibility evidence, requires empty console and page-error
evidence, and verifies owned browser and server cleanup. CI runs this path on
macOS and Windows.

From the repository root, after installing the website and Test Kit
dependencies plus the admitted standalone browser, run:

```bash
A3S_TEST_AGENT_BROWSER="$(command -v agent-browser)" \
  cargo test -p a3s-test-cli --test web_e2e \
  real_agent_browser_runs_the_website_testkit_suite \
  --locked -- --ignored --exact --nocapture
```

## Documentation version policy

`versions.mjs` is the source of truth for the public version selector. The
default entry must match the Rust workspace version. Every listed version owns
matching Chinese and English route trees. `publishedVersion` separately names
the release that installation commands may download. Main may stage the next
documentation version, but its homepage must keep installing the published
version and disclose that distinction until a release commit aligns both.

The active documentation lives under `docs/v0.17.0`. The `docs/v0.16.2` and
`docs/v0.15.0` directories are historical snapshots for previous contract
lines. New documentation work updates only the active version. When a release
changes a public action schema, provider protocol, CLI contract, or safety
boundary, archive the old directory before advancing the default version.

Before a tag can create a GitHub Release, the release preflight requires the
tag, Rust workspace version, default and published documentation versions,
dated changelog section, packaged Test Kit version, ordered snapshot metadata,
and both locale trees to agree. A tag is rejected while `publishedVersion`
still names an older stable release. Each snapshot records the Test Kit version
documented by that route tree. The preflight then runs the same formatting,
contract, build, and generated-site checks used by the documentation workflow.
Run the metadata gate locally with:

```bash
node ../scripts/check-release-metadata.mjs

# Only after publishedVersion and every installer example move to v0.17.0.
node ../scripts/check-release-metadata.mjs --tag v0.17.0
```

`npm run check:site` derives expected routes from the source trees, verifies
the default locale and version-pinned installers, and rejects broken internal
references in the generated site.
