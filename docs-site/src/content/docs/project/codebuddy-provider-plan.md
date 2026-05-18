---
title: CodeBuddy Provider Plan
description: Planned changes for supporting CodeBuddy Code CLI local conversation records in sivtr.
---

This document describes the planned CodeBuddy provider. CodeBuddy here means local agent transcripts produced by CodeBuddy Code / CodeBuddy CLI running in a terminal. It does not mean controlling the CodeBuddy IDE UI, and it does not mean parsing ordinary IDE logs.

The goal is to make CodeBuddy CLI transcripts work like existing Codex and Claude transcripts: a structured `sivtr` source that can be copied, filtered, searched, and picked.

## Target behavior

After the first implementation, these commands should work:

```bash
sivtr copy codebuddy
sivtr copy codebuddy out
sivtr copy codebuddy in
sivtr copy codebuddy tool
sivtr copy codebuddy all
sivtr copy codebuddy --pick
sivtr copy codebuddy --session <id>
sivtr hotkey-pick-agent --provider codebuddy
```

These commands should read CodeBuddy CLI JSONL transcripts from disk. Capturing terminal screen output remains covered by the existing pipe, run, and shell session features.

## Data source

Primary source:

```text
~/.codebuddy/projects/**/*.jsonl
```

The observed JSONL shape maps cleanly to `sivtr` agent blocks:

| CodeBuddy record | sivtr block |
| --- | --- |
| `type=message`, `role=user` | User |
| `type=message`, `role=assistant` | Assistant |
| `type=function_call` | ToolCall |
| `type=function_call_result` | ToolOutput |
| `sessionId` | session id |
| `cwd` | workspace match path |

The first version should not use these paths as primary session sources:

- `~/.codebuddy/traces/`
- `~/.codebuddy/logs/`
- `~/.codebuddy/.credentials.json`

`traces` and `logs` are better treated as future diagnostics. Credential files must not be read.

## Implementation scope

Add a core module:

```text
crates/sivtr-core/src/codebuddy.rs
```

The module should implement `AgentSessionProvider`:

- `list_recent_sessions`: scan CodeBuddy session directories and sort by modified time;
- `parse_session_file`: convert JSONL records into `AgentSession` and `AgentBlock`;
- `find_session_by_id`: resolve full `sessionId` values and id prefixes;
- `find_current_session`: prefer the newest non-empty CodeBuddy session whose `cwd` matches the current directory.

Register the provider:

- add `AgentProvider::CodeBuddy`;
- add a provider spec with `name = "CodeBuddy"` and `command_name = "codebuddy"`;
- export the `codebuddy` module from `sivtr-core`.

Wire CLI behavior:

- add `codebuddy` under the `copy` subcommands;
- dispatch it through `run_agent_copy(AgentProvider::CodeBuddy, ...)`;
- let `hotkey-pick-agent --provider codebuddy` work through the existing provider selection path.

## Configuration

Default source directory:

```text
~/.codebuddy/projects
```

Planned optional config:

```toml
[codebuddy]
session_dirs = ["/Users/dawn80s/.codebuddy/projects"]
```

An environment override can also be considered:

```bash
SIVTR_CODEBUDDY_SESSION_DIRS=/path/to/projects
```

The separator should match the existing Codex directory override rule: `:` on Unix and `;` on Windows.

## Parsing rules

`message` records:

- `role=user` reads `content[].input_text` or the shared content-text extraction path;
- `role=assistant` reads `content[].output_text` or the shared content-text extraction path;
- empty text does not create a block.

`function_call` records:

- use `name` as the tool label;
- format `arguments` as pretty JSON when possible;
- keep the raw string when it is not valid JSON.

`function_call_result` records:

- prefer `output.text`;
- fall back to `providerData.toolResult.content` when needed;
- the first version should create only ToolOutput blocks and should not chase external `tool-results/*.txt` files.

Metadata:

- `sessionId` becomes `AgentSession.id`;
- `cwd` becomes `AgentSession.cwd`;
- file modified time drives session-list ordering.

## Non-goals

The first version should not:

- control the CodeBuddy IDE UI;
- treat ordinary CodeBuddy IDE logs as conversations;
- read `.credentials.json`;
- expose `subagents/*.jsonl` as independent primary sessions;
- add CodeBuddy export or mirror commands;
- change existing Codex or Claude provider behavior.

`subagents` can be revisited later as attached tool content or a separate provider view.

## Test plan

Core tests:

- parse user and assistant messages;
- parse function calls and function call results;
- extract `sessionId` and `cwd`;
- skip empty content;
- tolerate trailing partial JSONL;
- choose the newest current-session match by `cwd`.

CLI tests:

- `sivtr copy codebuddy`
- `sivtr copy codebuddy out --print`
- `sivtr copy codebuddy --session <id>`
- `sivtr hotkey-pick-agent --provider codebuddy`
- unknown providers still fail.

Verification command:

```bash
cargo test --workspace
```

## Acceptance criteria

The first version is complete when:

- `sivtr copy codebuddy out --print` prints the latest CodeBuddy CLI assistant reply;
- `sivtr copy codebuddy --pick` shows CodeBuddy sessions and dialogue blocks in the picker;
- running inside a project prefers the CodeBuddy session whose `cwd` matches that project;
- `--session` supports numeric selection, full ids, and id prefixes;
- the default all-provider picker includes CodeBuddy;
- existing Codex and Claude tests do not regress.
