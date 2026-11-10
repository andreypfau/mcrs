---
name: mcrs-update-docs
description: Update the docs site docs-site/ of mcrs with what was merged into main since the last check - pages, status rows, the issue registry, the news, then a pass that removes signs of AI writing from the changed text. Load when someone asks to update the docs. Triggers - update the docs, refresh the docs, docs update, /mcrs-update-docs, обновить документацию, обнови доки.
---

# Update the mcrs docs

You update the site `docs-site/` with what was merged into `origin/main` since the last check. Nobody will re-read your edits line by line, so the rule is one: each thing is written in the one way the table in step 5 gives, and everything you are not sure of goes into the report, not into the text.

Before you start, read `docs-site/src/content/docs/en/contributing/docs.mdx` and `docs-site/README.md`. They are the contract of the site. If the contract and this file disagree on the syntax of a component, trust the code of the component in `docs-site/src/components/` and name the disagreement in the report.

## Limits

- You edit `docs-site/` only. Do not touch `docs/`, `PERF.md`, the root `README.md` or the code of the crates. Do not add comments to Rust code for the docs (`AGENTS.md`: "Default to zero comments"). The code fragment of an `Example` is set by the props of the component; see them in `docs-site/src/components/Example.astro`.
- Create a new page or section only by the procedure of step 4: an analyst agent, then the explicit consent of the user. Without these two steps you do not create, delete, rename or move pages, and you do not move text between them.
- Do not push and do not open a pull request. Commit to the working branch.
- Do not invent. Every statement has a source: a file in the repository, an issue, a pull request, `PERF.md`. No source, no statement.
- One run handles every change from the base to `origin/main`. If more than 15 pages are affected, show the list first and ask whether to go on.

## Step 0. Fetch the guide on AI writing

The guide "Signs of AI writing" of Wikipedia changes almost every day, so fetch a fresh copy at the start of every run. Use the raw wikitext, it has no HTML noise:

```bash
GUIDE=$(mktemp -t signs-of-ai-writing).txt
curl -sfL -A "mcrs-update-docs" "https://en.wikipedia.org/w/index.php?title=Wikipedia:Signs_of_AI_writing&action=raw" -o "$GUIDE"
wc -c "$GUIDE"     # about 180 KB in 2026-11; a few bytes means the fetch failed
```

Step 9 uses this file. If the fetch fails, say so in the report and run step 9 with the rules of step 7 only. Do not use a copy from an earlier run.

## Step 1. Sync

```bash
git fetch origin
git status --short          # the tree must be clean; if not, stop and ask
git ls-tree -d --name-only origin/main docs-site
```

- If `docs-site` is in `origin/main`, create the branch `docs/update-YYYY-MM-DD` from `origin/main`.
- If it is not (the docs still live in their own branch), stay in the current branch and run `git merge origin/main`. Do not rebase. On a conflict, stop and show it.
- Do not check out `main`. Another worktree can hold it.

Then `cd docs-site && npm ci`, and record where the run starts: `RUN_START=$(git rev-parse HEAD)`. Step 9 needs it.

## Step 2. Base and difference

The base is the oldest `verified.commit` among the pages in `docs-site/src/content/docs/en/`.

```bash
BASE=<the oldest verified.commit>
git log --oneline $BASE..origin/main | wc -l
git diff --name-status -M $BASE..origin/main -- . ':!docs-site'      # what changed, with renames
node docs-site/scripts/check-sources.mjs $(git diff --name-only $BASE..origin/main -- . ':!docs-site')
node docs-site/scripts/check-sources.mjs --stale                     # pages whose sources changed since their own verified.commit
node docs-site/scripts/check-issues.mjs                              # registry states that GitHub disagrees with
DATE=$(git show -s --format=%cs $BASE)
gh pr list -R andreypfau/mcrs --state merged --search "merged:>=$DATE" --limit 300 --json number,title,labels,closingIssuesReferences
gh issue list -R andreypfau/mcrs --state closed --search "closed:>=$DATE" --limit 300 --json number,title,labels,stateReason
gh issue list -R andreypfau/mcrs --state open --search "created:>=$DATE" --limit 300 --json number,title,labels
git tag --sort=-creatordate | head -5
```

Make a work list from this. It holds:

1. A page with a changed file in `sources` (named by `check-sources`).
2. A page that mentions an issue or a pull request whose state changed. Find it with `grep -rn "issue={N}\|pr={N}" docs-site/src`.
3. A row of `docs-site/src/data/status.yml` whose area a merged pull request touches.
4. A page with an `Example` whose file was renamed or deleted, or whose anchor text changed (the build fails by itself, but check before).
5. The references that mirror the whole code: `reference/env-vars` (a new or removed `MCRS_*`), `reference/commands` (a new command), `crates/overview` (a new, removed or renamed crate, and the fact `crates`), `reference/specs` (a new file in `docs/spec/`).
6. `project/news`: a new entry if there is a new tag, and an update of the section "since the last tag".
7. A changed `assets/minecraft/version.json`: go through `contributing/new-snapshot.mdx`, section "Docs checklist".

For each item read the diff (`git diff $BASE..origin/main -- <file>`) or the description of the pull request (`gh pr view N -R andreypfau/mcrs`). The commit title is not enough.

## Step 3. A decision for each page

The writing rules of steps 5 to 7 apply to every edit, this step included. Read them before the first edit.

For a page of the list, the answer is one of three:

- The text is still right. Then you change only `verified`.
- The text is out of date. You fix the text, EN and RU, then `verified`.
- You cannot tell. You do not touch the text, you do not move `verified`, and you write it into the report.

A change that does not alter the behavior a page describes (a refactor, a rename of a private item, tests) does not change the text. The docs grow only from what the reader sees: new behavior, a new public API, a new setting, a lifted limit, a closed issue.

On the way, collect the list of "homeless" items: new behavior or API from merged pull requests that has no place on any page, and pages that grew past 1500 words. Step 4 takes this list.

## Step 4. New pages and sections

Commit the edits of existing pages from step 3 first, then come here. If the homeless list is empty and no page grew past 1500 words, skip this step.

### 4.1. The analyst agent

You do not decide on the structure yourself. Start one agent (a subagent if your tool has them, read only) with this brief, with the data filled in:

```
You analyze the structure of the mcrs docs. Edit nothing, only read and answer.

Site: docs-site/src/content/docs/en/<group>/<slug>.mdx. The groups and their order are set in docs-site/astro.config.mjs (sidebar). The rules of the site: docs-site/src/content/docs/en/contributing/docs.mdx. A page has one kind (tutorial, how-to, reference, explanation) and a size of 1-4 screens.

Merged into main from <BASE> to <HEAD> and with no place in the docs:
<list: pull request, what it gives the reader, which files>

Pages that outgrew their size:
<list with word counts>

Read the headings and first paragraphs of all EN pages, and the whole text of the pages near the topic. Answer:
1. Is a new page needed? First check whether a section on an existing page solves it. A new page is justified if the material has its own reader question and its own kind, or if the receiving page would pass 4 screens.
2. Is a new sidebar section needed? It is justified by 3 or more pages with a shared audience that do not fit the current groups. One page does not justify a section.
3. What of the existing text must move there. A section or a page made only of new material is almost always a mistake: find the paragraphs and sections on existing pages that belong to the same topic and propose to move them. For each, name the file and the heading of the section.
4. What stays at the old place after the move: a link, one sentence, nothing.

Answer format, one block per proposal:
- Type: new page | new section | split a page | create nothing
- Path: en/<group>/<slug>.mdx, kind, place in the sidebar (after which page)
- The reader question it answers, in one sentence
- Why the existing pages are not enough
- Content: a list of sections. Each is marked "new: PR #N" or "moves from <file>#<heading>"
- What changes on the donor pages
- Which URLs stop existing
- Risk: what can go wrong
If nothing must be created, say so and explain where to add the material.
```

Check the answer yourself on two points before you bring it to the user: the files and headings it names exist, and the proposed path is free.

### 4.2. The consent of the user

Ask for consent for each proposal through your question tool, up to 4 proposals in one call. Before the question, write for each proposal:

- what is created: path, title, kind, place in the sidebar;
- why: the reader question and why the current pages are not enough;
- what goes in: a table "section, from where (new from PR #N, or moved from a page)";
- what changes on the old pages and which URLs disappear;
- what happens if the user says no: where the new material goes then.

Answer options: create as proposed, create without moving the old text, do not create and add to an existing page, postpone. Create nothing without the answer "create". Write a refusal or a postponement into the report with the proposal.

### 4.3. Creation

- A page is created in EN and RU at the same path. The frontmatter of the EN file follows the schema in `docs-site/src/content.config.ts`: `kind`, `audience`, `stability`, `tracking`, `sources`, `verified`, `sidebar.order`. The RU file carries only `title`, `description`, `sidebar` and `translated_from`.
- A move of text is a move, not a copy. The donor page keeps what the user approved (a link or one sentence). One fact lives in one place.
- Check every statement of the moved text against the code at `origin/main` and update it, since it moves anyway. Then add the new material.
- If a URL disappears (a page is renamed, deleted or split), add an entry to `redirects` in `docs-site/astro.config.mjs` and fix every link to the old address: `grep -rn "<old path>" docs-site/src`.
- A new sidebar section: a folder in `en/` and `ru/`, an entry in `sidebar` in `astro.config.mjs` with `translations.ru`, and the group name in the lists of groups in `contributing/docs.mdx` (en, ru) and `docs-site/README.md`.
- Link the new page from the pages near its topic (sections "What next") and from `index.mdx` if it is a new section. A page with no incoming links is not done.
- One commit per approved proposal: `docs(site): add a page about <...> and move <...> into it`.

## Step 5. How to write. One way for each thing

| What | The only way | Not like this |
|---|---|---|
| Issue | `<Status issue={N} />`. The state comes from `docs-site/src/data/issues.yml` | `#N` as text, `[#N](https://github.com/...)`, "issue 78", the state in words next to the link, `state=` next to `issue=` (the build fails) |
| Pull request | `<Status pr={N} state="works" />` if the pull request merged the thing described | `[#N](.../pull/N)`, `[PR #N](...)` |
| A file of the repository | `[path](https://github.com/andreypfau/mcrs/blob/main/<path>)`, the link text is the path as in the repository | relative links, links to a branch or a commit |
| Another page of the site | `[Exact title of the page](/docs/en/<group>/<slug>/)`, in RU `/docs/ru/...`. The link text equals the `title` of the target page | relative paths, own names ("Crates overview" for the page "Crate map") |
| Code from the repository | `<Example file="..." from="..." to="..." />`, cut by anchors in the code text | a hand-written ` ``` ` block of repository code, line numbers in prose |
| Code not from the repository (Paper, Gradle) | a ` ```java ` block and the line "illustrative, not mcrs code" | - |
| Something that does not exist yet, part of a page | `<Aside type="caution" title="WIP">` with a `Status` inside | italics, "(WIP)", "TODO", "coming soon" in the text |
| Something that does not exist yet, a whole page | `stability: wip` and `tracking: [N]` in the EN frontmatter, no Aside at the top that repeats it | an apology Aside as the first block of the page |
| A count that two pages repeat (crates, registries, commands) | `{fact('id')}` from `docs-site/src/data/facts.yml`, re-measured with its `how` | a number from memory, "about N", a change in one place |
| Game version | `{fact('snapshot')}` | the version from memory |
| A performance number | a number from `PERF.md` with the machine and the scenario next to it, and a link to the section | own measurements, comparisons with vanilla, Paper or other servers |
| A command for the reader | only one you ran in this run and that worked | "we did not run this command", a command not run |
| The state of an area | a row in `docs-site/src/data/status.yml` by the `status` schema of `docs-site/src/content.config.ts`: `title`, `works` and `missing` (each with `en` and `ru`), `state`, `issues`, `evidence`, `verified` | an own state table on a page |

### Status for an issue

The state lives in `issues.yml`. In the text you write `<Status issue={N} />`, and the component takes the badge from the registry. A new issue first gets a line in the registry. A change of state is one line in the registry.

There is no form without a badge anywhere, the lists "what to follow" included. Do not write a number as plain text (`#N`, "PR #N", "issue #N"), and do not put "PR" before a pull request badge. Do not write the state in words next to the badge ("is open", "(closed)", "a closed decision"): the badge shows it.

The badge stands next to the statement it belongs to. In one paragraph an issue gets its badge once, at the first mention.

### How to find the state

Run `gh issue view N -R andreypfau/mcrs --json state,stateReason,labels,title,closedByPullRequestsReferences` and apply the table. Memory and the text of the old page are not sources.

| State | Condition |
|---|---|
| `works` | the issue is closed as completed, and a test or a section of `PERF.md` shows it. Name it in `sources` |
| `partial` | part is merged (a merged pull request or code exists), and the issue is open or the rest moved to another issue |
| `wip` | the issue is open, and there is no code for it or the code is not merged |
| `undecided` | the label `needs-decision`, or there is no issue at all |
| `declined` | an issue with the label `decision` closed with a refusal, or closed as not planned |

A closed issue without a test and without a measurement gets `partial`, not `works`, and goes into the report.

## Step 6. Frontmatter and translation

- `verified` of an EN page after the check: `snapshot` from `version.json`, `commit` is the short hash of `origin/main` you checked against, `date` is today. Move it only on pages you really checked against the code. Do not move it in bulk.
- `docs_commit` and `docs_commit_date` in `docs-site/src/data/facts.yml` (the banner on the home page): the short hash of `origin/main` that the whole run checked against, and its date from `git show -s --format=%cs <hash>`. Move them in every run, even if no page changed. `npm test` fails if the date does not match the commit.
- `sources`: add the files the new text relies on. Replace a renamed file with its new path. Take paths from `git ls-files`, not from the text of an issue.
- `stability` and `tracking`: when the tracked issue is closed and the text no longer changes, the page becomes `stable` and `tracking` becomes empty. A page that is not `stable` always has `tracking`.
- The RU page is edited in the same commit as the EN page: the same structure, the same order, no content that EN does not have. `translated_from` gets the same hash as `verified.commit`.
- A RU file carries only `title`, `description`, `sidebar` and `translated_from`. The rest is inherited from EN, and any other field fails the build.

## Step 7. Content and style

Each rule below closes a mistake that the site already had.

Content:
- One fact sounds the same on the whole site. Before you write a number, a state or a limit, find its earlier mentions (`grep -rn` by a keyword in `docs-site/src`) and bring them to one value in the same commit.
- No judgements of other projects and no comparisons with them, the page `start/philosophy` included. Do not write that another server is slow, that forks "sell" something, or that someone "is stuck on one thread". You may name Paper, Folia, Minestom and Velocity, neutrally and only about their public API.
- The present tense only for what is in the code at `origin/main`. A goal or an example from the future is written as a goal, with an issue badge in the same sentence, not a paragraph below.
- No absolute claims without a number: "costs nothing", "never", "always", "any". Either a number from `PERF.md`, or softer wording.
- A tutorial leads the reader by the hand: the reader writes the code and runs the command, and the page shows what they will see. If a step can only be shown on a test of the repository, it is a how-to or an explanation, not a tutorial.
- A command on a page was run by you in this run. Do not write "we did not run this": run it, or do not add the command.
- A caveat lives in one place. Do not repeat the same paragraph word for word on several pages (no plugin loader, the benchmark machine, "before the alpha"): write it on one page and link to it. The exception is the warning about the missing authentication next to each command that starts the server.
- A page does not start with an apology. The first screen answers the question of the reader. If the whole page is not ready, it is `stability: wip` in the frontmatter, and the banner draws itself.
- Do not tell the reader how the docs were written: "this page copies", "read on 2026-11-09 from gh pr list", "(README, section such and such)". Sources live in the frontmatter `sources`.
- The size of a page is 1-4 screens, about 1500 words. A page that grows past it is a candidate for a split, which is step 4.
- A list does not merge into one sentence. Four items or more are a list or a table.

English:
- Plain language for a non-native reader: short sentences, active voice, one thought per sentence, at most 30 words.
- American spelling: behavior, color, license. Two words: data pack, run time.
- ASCII hyphen only. No long dashes, no typographic quotes, no ellipsis character.
- No lists of the form "- **Title.** text". A table or plain sentences.
- A paragraph is one line, no hard wrap.
- Do not start a page or a section with "This page ...". Do not write "the owner", "the author", "we did not run". "We" only about actions of the project.

Russian:
- The way a developer explains to a colleague: meaning over the structure of the English sentence, verbs instead of chains of nouns.
- No "является", "осуществлять", "данный". No "вы" where the phrase works without it.
- Always "е", never the letter with two dots.
- Do not translate settled terms: тик, чанк, крейт, плагин, снапшот, датапак, issue, PR. Issue titles stay in English.

Both languages: clean room. No names of classes, methods or fields from the code of the game, except `ResourceKey`, `TagKey`, `BlockPos`, `SectionPos`, `ColumnPos`, `PalettedContainer`.

## Step 8. Checks before a commit

All commands run from `docs-site/`. Any failure is fixed, not worked around.

```bash
npm test
npm run check
npm run build
grep -rnE '[—–…“”‘’«»]|ё|Ё' src/content/docs src/data src/content/i18n                                                        # empty
grep -rnE '(^|[^[{=/&a-zA-Z0-9`#])#[0-9]{2,4}\b' src/content/docs                                                        # empty: an issue or PR number only through Status
diff <(cd src/content/docs/en && find . -name '*.mdx' | sort) <(cd src/content/docs/ru && find . -name '*.mdx' | sort)     # empty
```

`npm test` already fails on `state=` next to `issue=`, on bare issue and pull request links, on a typed snapshot id and on a missing `sources` path. For each changed EN and RU pair, compare the number of headings, `<Example`, `<Status`, `<Aside` and table rows: it is the same.

## Step 9. Remove signs of AI writing from the changed text

This pass covers only the text this run added or changed, never the whole site. The scope is the diff from `RUN_START`, commits and working tree together:

```bash
git diff $RUN_START -- docs-site/src/content/docs docs-site/src/data/status.yml docs-site/src/content/i18n > "$(mktemp -t docs-run).diff"
```

Give the pass to one agent with no context of this run (a subagent if your tool has them; otherwise do it yourself after you re-read only the guide and the diff). The brief:

```
You remove signs of AI writing from the text that one docs update of mcrs changed. Work in <worktree>, branch <branch>.

Read first:
- <GUIDE>: the current wikitext of Wikipedia:Signs_of_AI_writing. Use the sections under "Content", "Language and grammar", "Style" and "Communication intended for the user". Skip the parts about Wikipedia markup, citations and article creation.
- <DIFF>: the change of this run. Only the added lines (+) are in scope.
- docs-site/src/content/docs/en/contributing/docs.mdx: the rules of the site. They win over the guide on a conflict.

For each added line in the diff, find the signs the guide lists (inflated significance, AI vocabulary, negative parallelisms, rule of three, vague attributions, superficial -ing analyses, filler, promotional tone, sycophantic or chatty phrases, and so on) and rewrite the line in plain words. Keep:
- every fact, number, link, component (<Status>, <Example>, <Aside>, {fact(...)}) and frontmatter field as it is;
- the EN and RU versions in step: a change in one language gets the matching change in the other;
- the line count of tables and lists.
Do not touch lines the diff does not add. Do not rewrite text that has no sign of AI writing.

Then run from docs-site/: npm test && npm run build. Commit once: docs(site): remove signs of AI writing from the text of this docs update. Report each changed line as "file: before -> after, sign from the guide", at most 40 lines.
```

Review the report. Revert any change that altered a fact or broke the EN and RU match, and run step 8 again.

## Step 10. Commits

One commit per logical cause (one pull request or one topic), not one per file and not one for everything. The message is in English in the style of the repository, and the body names the pull request or issue that caused it:

```
docs(site): describe the join event that #701 added
```

## Step 11. Report

End the answer with a report in this form:

- Base and head: `<BASE>..<HEAD>`, the number of commits, the number of pull requests looked at.
- A table "page, what changed, cause (pull request or issue)".
- Issues whose state changed: number, before, after, by which sign.
- Pages checked without text edits (only `verified` moved).
- New pages and sections: what the analyst proposed, what the user approved, refused or postponed, what was created and what moved where.
- The AI writing pass: the size of the fetched guide, the number of changed lines, and the changes you reverted.
- What is not done and needs a decision of the user: closed issues without a test, places where you could not tell whether the text is right, disagreements of the contract with the code of the components.
- The last lines of `npm run build` and the result of the checks of step 8.

Ask the user your questions in one batch at the end, not on the way. On the way ask only when you cannot go on without an answer (a dirty tree, a merge conflict, more than 15 pages) and for the consent to create a page or a section in step 4.
