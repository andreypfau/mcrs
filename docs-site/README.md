# mcrs docs site

Documentation for people, built with Astro 6 and Starlight. Published at `/docs/` next to the landing page in `site/`.

## Run locally

```sh
cd docs-site
npm install
npm run dev
```

Open http://localhost:4321/docs/. `npm run build` writes the static site to `docs-site/dist`, the same command CI runs.

## Layout

- `src/content/docs/en/<group>/<slug>.mdx` - English pages (main language). Groups: start, plugins, concepts, crates, server, client, reference, contributing, project.
- `src/content/docs/ru/` - Russian pages at the same paths.
- `src/components/` - Status, StatusRows, Example (cuts code by text anchors with `src/lib/anchor.mjs`), Demo.
- `src/data/status.yml` - data for the StatusRows component.
- `src/data/facts.yml` and `src/lib/facts.mjs` - counts and the snapshot id that several pages repeat, read with `fact('id')` in MDX and `{id}` in status.yml.
- `src/content.config.ts` - frontmatter schema. A wrong field breaks the build.
- `src/lib/meta.ts` - `pageMeta`: the page metadata for the banner and the footer. A Russian page takes it from its English pair and fails the build if it sets one of the inherited fields.
- `scripts/check-sources.mjs` - warns about pages whose `sources` changed in a PR. It never fails the build. Run `npm run check-sources` locally. With `--stale` it prints a Markdown report of English pages whose `sources` changed since `verified.commit`, Russian pages that lag behind English, and missing `sources` paths. The `docs` workflow runs it every Monday.
- `scripts/lib/pages.mjs` - loads the pages and computes those reports. `npm test` also fails on a `sources` path that does not exist.

The sidebar is built from the directories. `sidebar.order` in the frontmatter sets the order inside a group.

## Writing rules

Read [Writing docs](/docs/en/contributing/docs/) (source: `src/content/docs/en/contributing/docs.mdx`).

## Docs in the same PR

A change that alters behavior described on a page updates that page in the same PR. When a file listed in a page's `sources` changes, the `docs` workflow adds a warning on that page. Update the page and its `verified` block, or explain in the PR why no change is needed.

## CI

- `.github/workflows/docs.yml` - build and source check on pull requests.
- `.github/workflows/pages.yml` - on main, builds `site/` and `docs-site/` and deploys one artifact (`site/` at the root, the docs under `docs/`).
