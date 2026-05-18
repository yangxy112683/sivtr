---
title: CodeBuddy Code
description: Use sivtr to copy, inspect, and reuse local CodeBuddy Code conversation records.
---

`sivtr` can read local CodeBuddy Code / CodeBuddy CLI JSONL transcripts and turn them into the same reusable blocks as Codex and Claude sessions: user messages, assistant replies, tool calls, and tool outputs.

Use this when you are working in CodeBuddy Code and want to quickly reuse the latest answer, inspect a tool result, or pick an older conversation block without opening the raw transcript file.

## Data source

By default, `sivtr` scans:

```text
~/.codebuddy/projects
```

It reads project session JSONL files and chooses the newest non-empty session whose `cwd` matches your current directory. If no matching session exists, it falls back to the newest non-empty CodeBuddy session.

`sivtr` does not use these paths as primary sessions:

- `~/.codebuddy/traces/`
- `~/.codebuddy/logs/`
- `~/.codebuddy/.credentials.json`
- `~/.codebuddy/projects/<project>/<session>/subagents/*.jsonl`

## Copy the latest CodeBuddy reply

Run this in the same project directory where you use CodeBuddy Code:

```bash
sivtr copy codebuddy out --print
```

This copies the latest assistant reply to the clipboard and prints it to stdout for inspection. The currently installed version supports this command.

## Supported commands

The CodeBuddy provider lives under the `copy` command:

```bash
sivtr copy codebuddy [OPTIONS] [N|A..B] [COMMAND]
```

| Command | Meaning |
| --- | --- |
| `sivtr copy codebuddy` | Copy the latest completed user + assistant turn |
| `sivtr copy codebuddy out` | Copy the latest assistant reply |
| `sivtr copy codebuddy in` | Copy the latest user message |
| `sivtr copy codebuddy tool` | Copy the latest tool output |
| `sivtr copy codebuddy all` | Copy the whole parsed session |
| `sivtr copy codebuddy --pick` | Open the CodeBuddy session and dialogue picker |
| `sivtr copy codebuddy --session 2` | Read the 2nd newest selectable CodeBuddy session |
| `sivtr copy codebuddy --session <id>` | Read by full session id or id prefix |
| `sivtr hotkey-pick-agent --cwd . --provider codebuddy` | Open the CodeBuddy picker for the current directory |

These options can be combined with the modes above:

| Option | Meaning |
| --- | --- |
| `--print` | Print the copied text to the terminal; useful for inspection |
| `--regex <PATTERN>` | Keep only lines matching a regex |
| `--lines <SPEC>` | Keep selected lines, for example `10:80` or `1,3,8:12` |
| `--pick` | Open the interactive picker |
| `--session <N|ID>` | Select a session by index, full id, or id prefix |

Useful combinations:

```bash
sivtr copy codebuddy out --print       # inspect and copy the latest assistant reply
sivtr copy codebuddy tool --print      # inspect and copy the latest tool output
sivtr copy codebuddy all --print       # inspect and copy the whole parsed session
sivtr copy codebuddy all --lines 1:40  # copy only the first 40 lines
sivtr copy codebuddy tool --regex error --print
```

Notes:

- The correct option is `--print`, not `=--print`.
- The correct mode is `out`, not `output`.
- Top-level `sivtr codebuddy ...` is not supported; use `sivtr copy codebuddy ...`.

`out` only returns assistant text. It does not mix in tool output. Use `tool` when you specifically need the latest tool result.

## Pick interactively

Open the picker when you want to browse sessions or choose older blocks:

```bash
sivtr copy codebuddy --pick
```

Inside the picker:

- choose a CodeBuddy session first;
- then choose one or more dialogue blocks;
- press `t` to open the Vim-style view for the highlighted block.

You can also start from a specific content mode:

```bash
sivtr copy codebuddy out --pick
sivtr copy codebuddy tool --pick
```

## Select an older session or block

Use `--session N` for the Nth newest selectable session, or pass a session id / id prefix:

```bash
sivtr copy codebuddy --session 2
sivtr copy codebuddy --session cb-session-prefix
sivtr copy codebuddy out --session 3 --print
```

Selectors after the mode select blocks inside the chosen session:

```bash
sivtr copy codebuddy 2
sivtr copy codebuddy 2..4
sivtr copy codebuddy all --lines 1:40
```

## Filter copied text

Filters run after the selected text is assembled:

```bash
sivtr copy codebuddy tool --regex error --print
sivtr copy codebuddy all --lines 10:80 --print
```

Use `--print` when you want to inspect the result in the terminal instead of relying only on the clipboard.

## Use from CodeBuddy Code workflows

Useful one-liners while working with CodeBuddy Code:

```bash
# Review the last CodeBuddy answer in the terminal.
sivtr copy codebuddy out --print

# Copy the last tool output, often useful after failed builds or tests.
sivtr copy codebuddy tool --print

# Pick a previous answer from the current project.
sivtr copy codebuddy --pick

# Open the current-project CodeBuddy picker directly.
sivtr hotkey-pick-agent --cwd . --provider codebuddy
```

For the VS Code bridge, set the extension arguments to use CodeBuddy only if you do not want the all-provider picker:

```json
{
  "sivtr.args": ["hotkey-pick-agent", "--cwd", ".", "--provider", "codebuddy"]
}
```

## Configure extra transcript directories

If your CodeBuddy transcript tree is outside the default location, add it to the config:

```toml
[codebuddy]
session_dirs = ["/path/to/codebuddy/projects"]
```

Or set an environment variable:

```bash
SIVTR_CODEBUDDY_SESSION_DIRS=/path/to/projects
```

Use `:` to separate multiple paths on Unix/macOS and `;` on Windows.

## Troubleshooting

If `sivtr copy codebuddy out --print` prints nothing or reports that no session was found:

1. Run CodeBuddy Code in the project first so a local transcript exists.
2. Run the command from the same project directory so `cwd` matching can find the session.
3. If transcripts are stored elsewhere, configure `[codebuddy].session_dirs` or `SIVTR_CODEBUDDY_SESSION_DIRS`.
4. Use `sivtr copy codebuddy --pick` to browse all selectable CodeBuddy sessions.
