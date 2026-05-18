---
title: CodeBuddy provider 计划
description: 让 sivtr 支持 CodeBuddy Code CLI 本地对话记录的改造计划。
---

这份文档描述计划中的 CodeBuddy provider。这里的 CodeBuddy 指 CodeBuddy Code / CodeBuddy CLI 在命令行中运行时产生的本地 agent 对话记录，不是 CodeBuddy IDE 的界面控制，也不是普通 IDE 日志解析。

目标是让 CodeBuddy CLI transcript 像现有 Codex 和 Claude transcript 一样成为 `sivtr` 的结构化来源。

## 目标行为

第一版完成后，预期支持这些命令：

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

这些命令应该读取 CodeBuddy CLI 的本地 JSONL transcript，而不是捕获当前终端屏幕输出。终端输出捕获仍然由现有的 pipe、run 和 shell session 功能处理。

## 数据源

主数据源：

```text
~/.codebuddy/projects/**/*.jsonl
```

第一版只读取主会话 JSONL。实现时必须排除：

```text
~/.codebuddy/projects/<project>/<session>/subagents/*.jsonl
```

也就是说，`list_recent_sessions` 不能直接无条件复用递归 JSONL 扫描结果；如果复用共享扫描函数，必须在 CodeBuddy provider 内过滤掉路径中包含 `/subagents/` 的文件。

已观察到的 JSONL 结构适合直接映射到 `sivtr` 的 agent block：

| CodeBuddy 记录 | sivtr block |
| --- | --- |
| `type=message`, `role=user` | User |
| `type=message`, `role=assistant` | Assistant |
| `type=function_call` | ToolCall |
| `type=function_call_result` | ToolOutput |
| `sessionId` | session id |
| `cwd` | workspace 匹配路径 |

第一版不把这些目录作为主数据源：

- `~/.codebuddy/traces/`
- `~/.codebuddy/logs/`
- `~/.codebuddy/.credentials.json`
- `~/.codebuddy/projects/<project>/<session>/subagents/`

`traces` 和 `logs` 更适合后续诊断，不应作为会话 picker 的主要内容来源。凭据文件不应读取。

## 实现范围

新增核心模块：

```text
crates/sivtr-core/src/codebuddy.rs
```

该模块实现 `AgentSessionProvider`：

- `list_recent_sessions`：扫描 CodeBuddy session 目录，按修改时间排序；
- `parse_session_file`：把 JSONL 记录转换成 `AgentSession` 和 `AgentBlock`；
- `find_session_by_id`：支持按完整 `sessionId` 或前缀查找；
- `find_current_session`：优先匹配当前 `cwd` 下最新的非空 CodeBuddy 主会话；如果当前 `cwd` 没有匹配会话，则退回到最新的全局 CodeBuddy 主会话。

注册 provider：

- 在 `AgentProvider` 增加 `CodeBuddy`；
- 在 provider 注册表增加 `name = "CodeBuddy"` 和 `command_name = "codebuddy"`；
- 在 `sivtr-core` 导出 `codebuddy` 模块。

接入 CLI：

- 在 `copy` 子命令中增加 `codebuddy`；
- 在命令分发中接到 `run_agent_copy(AgentProvider::CodeBuddy, ...)`；
- 让 `hotkey-pick-agent --provider codebuddy` 通过现有 provider 选择逻辑工作。

## 配置

默认读取：

```text
~/.codebuddy/projects
```

第一版需要增加可选配置：

```toml
[codebuddy]
session_dirs = ["/Users/dawn80s/.codebuddy/projects"]
```

第一版同时支持环境变量：

```bash
SIVTR_CODEBUDDY_SESSION_DIRS=/path/to/projects
```

环境变量的分隔符应和现有 Codex 目录覆盖规则保持一致：Unix 使用 `:`，Windows 使用 `;`。

## 解析规则

第一版只解析这些对话相关事件：

- `message`
- `function_call`
- `function_call_result`

这些事件静默忽略，不生成 block：

- `summary`
- `file-history-snapshot`
- `ai-title`

未知 `type` 也静默忽略。CodeBuddy 本地格式可能扩展，`sivtr copy codebuddy out` 不应因为新增内部事件而失败。

`message` 记录：

- `role=user` 使用 `content[].input_text` 或通用文本提取；
- `role=assistant` 使用 `content[].output_text` 或通用文本提取；
- 空文本不生成 block。

`function_call` 记录：

- `name` 作为 tool label；
- `arguments` 尽量格式化成 pretty JSON；
- 无法解析为 JSON 时保留原始字符串。

`function_call_result` 记录：

- 按以下优先级提取文本：
  1. `output.text`
  2. `providerData.toolResult.content`
  3. `pretty_json_value(output)`
  4. `pretty_json_value(providerData.toolResult)`
- 第一版只生成 ToolOutput block，不额外追踪外部 `tool-results/*.txt`。

元数据：

- `sessionId` 写入 `AgentSession.id`；
- `cwd` 写入 `AgentSession.cwd`；
- 文件修改时间用于 session 列表排序。

选择语义：

- `sivtr copy codebuddy out` 只复制最后一条 assistant message，不混入 tool output；
- `sivtr copy codebuddy tool` 只复制最后一个 tool output；
- `sivtr copy codebuddy all` 只包含解析后的 User、Assistant、ToolCall、ToolOutput block，不包含 `summary`、`file-history-snapshot`、`ai-title`；
- `sivtr copy codebuddy --pick` 第一版使用现有通用标题逻辑，不额外读取 `ai-title` 作为 picker 标题。

## 不做的事情

第一版明确不做：

- 不控制 CodeBuddy IDE UI；
- 不把 CodeBuddy IDE 普通日志当成会话；
- 不读取 `.credentials.json`；
- 不把 `subagents/*.jsonl` 作为独立主会话展示；
- 不新增 CodeBuddy export/mirror 命令；
- 不新增 `sivtr codebuddy export`；
- 不让 `summary`、`file-history-snapshot`、`ai-title` 进入 `all`；
- 不改变现有 Codex 和 Claude provider 行为。

`subagents` 可以后续再评估：要么作为主会话的附属工具内容，要么作为单独 provider 视图，但不进入第一版范围。

## 测试计划

核心测试：

- 解析 user / assistant message；
- 解析 function call / function call result；
- 提取 `sessionId` 和 `cwd`；
- 空内容不生成 block；
- trailing partial JSONL 容错；
- 按当前 `cwd` 选择最新匹配会话。
- 当前 `cwd` 无匹配时退回最新全局主会话；
- `*/subagents/*.jsonl` 不进入 session 列表；
- `summary`、`file-history-snapshot`、`ai-title` 不生成 block；
- 未知事件不报错；
- `function_call.arguments` 合法 JSON 时格式化，非法 JSON 时保留原字符串；
- `function_call_result` 按既定优先级提取文本；
- `[codebuddy].session_dirs` 会追加扫描目录；
- `SIVTR_CODEBUDDY_SESSION_DIRS` 支持 Unix/macOS 的 `:` 和 Windows 的 `;` 分隔符。

CLI 测试：

- `sivtr copy codebuddy`
- `sivtr copy codebuddy out --print`
- `sivtr copy codebuddy in --print`
- `sivtr copy codebuddy tool --print`
- `sivtr copy codebuddy all --print`
- `sivtr copy codebuddy --session <id>`
- `sivtr hotkey-pick-agent --provider codebuddy`
- 未知 provider 仍然报错。

验证命令：

```bash
cargo test --workspace
```

本机安装和真实数据验收：

```bash
cargo install --path . --force
sivtr copy codebuddy out --print
```

## 验收标准

第一版完成时，应满足：

- `sivtr copy codebuddy out --print` 能输出最新 CodeBuddy CLI 助手回复；
- `sivtr copy codebuddy out --print` 只输出最后一条 assistant message，不包含 tool output；
- `sivtr copy codebuddy tool --print` 能输出最后一个 tool output；
- `sivtr copy codebuddy all --print` 不包含 `summary`、`file-history-snapshot`、`ai-title`；
- `sivtr copy codebuddy --pick` 能在 picker 中展示 CodeBuddy 会话和对话块；
- 当前目录下运行时，优先匹配 `cwd` 对应的 CodeBuddy session；
- 当前目录没有匹配会话时，退回最新全局 CodeBuddy 主会话；
- `--session` 能按序号、完整 id 或 id 前缀选择会话；
- `sivtr` 默认入口的 all-provider picker 能包含 CodeBuddy；
- `*/subagents/*.jsonl` 不会出现在主会话列表中；
- 现有 Codex / Claude 测试不回退。
