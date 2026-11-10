# Contributing to mcrs

The full guide is at https://mc.rs/docs/en/contributing/ (source: `docs-site/src/content/docs/en/contributing/`).

- Pick a role: specification author, implementer, parity tester, docs and translation. One person does not take both author and implementer for the same behavior.
- Read [docs/clean-room.md](docs/clean-room.md) before your first PR. If you have read the game's code about a behavior, write its specification, and leave the implementation to someone else. Do not put names of the game's classes, methods or fields in code, comments, issues or commit messages.
- A new behavior takes two issues: one for the specification in `docs/spec/`, one for the implementation that cites the rule IDs.
- Run the tests before a PR:

  ```bash
  cargo nextest run -p <crate>        # one crate
  cargo nextest run --workspace       # everything, before a commit
  cargo nextest run --profile full    # plus the exhaustive sweeps
  ```

- Docs go in the same PR. If you change behavior that a page of `docs-site/` describes, update the page. The `docs` workflow warns about pages whose `sources` you touched.
- Questions: [Discord](https://discord.gg/ged3nMRzwG) or the issue you work on.
