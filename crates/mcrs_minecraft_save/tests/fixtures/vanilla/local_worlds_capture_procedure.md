# Capture procedure: `local_worlds.json`

The game writes this file. Nobody edits it by hand.

## Capture

```sh
cargo run -p mcrs_minecraft_update -- recapture local_worlds
```

The task runs no server and no client. It makes its folders in a temporary directory and removes
them at the end. It runs on macOS or Linux. Two runs on one machine write the same bytes. The lock cases start a second Java
process of the same task, which takes or tests the lock of the game.

## Cases

The order in the file is the order of this list. `docs/spec/client-world-screens.md` and
`docs/spec/persistence.md`, in their sections "Experiments", state the rules that each case
confirms.

| Key | Cases |
| - | - |
| `folder_ids` | a world name and the folders that exist before the name is given; the folder id of the game |
| `seeds` | a text of the seed box; its seed, or `null` for a random seed |
| `summaries` | a saves folder with 10 entries, sorted by folder; each entry that the list of the game shows |
| `lock` | the bytes of `session.lock`, the release, a second process that holds or tests the lock, a missing file and a file that the process cannot write |
| `backup` | a world with only `level.dat_old`, after the game opens it |
| `delete` | a delete while a second process holds the lock, and after it releases the lock |

Each `level.dat` holds `DataVersion` and `Version` of the game. Its `LastPlayed` is 1000. Each
file of `summaries` has the modification time 1700000000000 ms.

## What the fixture cannot pin

- The test of a folder name is the test of the file system. The capture ran on macOS, with no
  regard to case. A name that differs only in case is not a case.
- A lock of another kind, for example `flock`, is not a case. Java cannot take it.
- A saves folder that cannot be listed, and a refusal of a folder name other than an existing
  folder, are not cases.
- The kind of lock and the effect of the close of a second handle are those of macOS and Linux.
  Windows can differ. The task does not run on Windows: the lock of the game there denies the
  read of `session.lock` through a second handle, so the task stops with an error at its start.
