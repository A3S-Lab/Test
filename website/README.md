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

## Documentation version policy

`versions.mjs` is the source of truth for the public version selector. The
default entry must match the Rust workspace version. Every listed version owns
matching Chinese and English route trees.

The active documentation lives under `docs/v0.16.2`. The `docs/v0.15.0`
directory is a historical snapshot for the previous contract line. New
documentation work updates only the active version. When a release changes a
public action schema, provider protocol, CLI contract, or safety boundary,
archive the old directory before advancing the default version.

`npm run check:site` derives expected routes from the source trees, verifies
the default locale and version-pinned installers, and rejects broken internal
references in the generated site.
