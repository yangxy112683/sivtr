---
title: CodeBuddy Code
description: 使用 sivtr 复制、查看和复用本地 CodeBuddy Code 对话记录。
---

`sivtr` 可以读取 CodeBuddy Code / CodeBuddy CLI 的本地 JSONL transcript，并把它们转换成和 Codex、Claude 会话一致的可复用块：用户消息、助手回复、工具调用和工具输出。

当你在 CodeBuddy Code 中工作，想快速复用最新回答、查看工具结果，或从历史对话中挑选某一段内容时，可以使用这组命令。

## 数据来源

默认扫描：

```text
~/.codebuddy/projects
```

`sivtr` 会读取 project session JSONL 文件，并优先选择 `cwd` 与当前目录匹配的最新非空会话。如果当前目录没有匹配会话，会退回到最新的非空 CodeBuddy 会话。

这些路径不会作为主会话读取：

- `~/.codebuddy/traces/`
- `~/.codebuddy/logs/`
- `~/.codebuddy/.credentials.json`
- `~/.codebuddy/projects/<project>/<session>/subagents/*.jsonl`

## 复制最新 CodeBuddy 回复

在使用 CodeBuddy Code 的同一个项目目录下执行：

```bash
sivtr copy codebuddy out --print
```

这会把最新助手回复复制到剪贴板，并打印到 stdout 方便检查。当前已安装版本支持该命令。

## 支持的命令清单

CodeBuddy provider 挂在 `copy` 子命令下，完整形式是：

```bash
sivtr copy codebuddy [OPTIONS] [N|A..B] [COMMAND]
```

| 命令 | 作用 |
| --- | --- |
| `sivtr copy codebuddy` | 复制最近一轮用户消息 + 助手回复 |
| `sivtr copy codebuddy out` | 复制最近助手回复 |
| `sivtr copy codebuddy in` | 复制最近用户消息 |
| `sivtr copy codebuddy tool` | 复制最近工具输出 |
| `sivtr copy codebuddy all` | 复制整个解析后的会话 |
| `sivtr copy codebuddy --pick` | 打开 CodeBuddy 会话和对话块选择器 |
| `sivtr copy codebuddy --session 2` | 读取第 2 新的可选 CodeBuddy 会话 |
| `sivtr copy codebuddy --session <id>` | 按完整 session id 或 id 前缀读取会话 |
| `sivtr hotkey-pick-agent --cwd . --provider codebuddy` | 直接打开当前目录的 CodeBuddy picker |

这些选项可以和上面的模式组合：

| 选项 | 作用 |
| --- | --- |
| `--print` | 复制后同时打印到终端，推荐调试时使用 |
| `--regex <PATTERN>` | 只保留匹配正则的行 |
| `--lines <SPEC>` | 只保留指定行，如 `10:80` 或 `1,3,8:12` |
| `--pick` | 打开交互式选择器 |
| `--session <N|ID>` | 指定会话序号、完整 id 或 id 前缀 |

常用组合：

```bash
sivtr copy codebuddy out --print       # 查看并复制最近助手回复
sivtr copy codebuddy tool --print      # 查看并复制最近工具输出
sivtr copy codebuddy all --print       # 查看并复制整个解析会话
sivtr copy codebuddy all --lines 1:40  # 只复制前 40 行
sivtr copy codebuddy tool --regex error --print
```

注意：

- 正确选项是 `--print`，不是 `=--print`。
- 正确模式是 `out`，不是 `output`。
- 当前不支持顶层 `sivtr codebuddy ...`，请使用 `sivtr copy codebuddy ...`。

`out` 只返回助手文本，不会混入 tool output。如果需要最新工具结果，请用 `tool`。

## 交互式选择

想浏览会话或挑选更早的块时，打开 picker：

```bash
sivtr copy codebuddy --pick
```

在 picker 中：

- 先选择一个 CodeBuddy 会话；
- 再选择一个或多个对话块；
- 按 `t` 打开高亮块的 Vim 风格视图。

也可以从指定内容模式进入：

```bash
sivtr copy codebuddy out --pick
sivtr copy codebuddy tool --pick
```

## 选择更早的会话或块

`--session N` 选择第 N 新的可选会话，也可以传 session id 或 id 前缀：

```bash
sivtr copy codebuddy --session 2
sivtr copy codebuddy --session cb-session-prefix
sivtr copy codebuddy out --session 3 --print
```

模式后的 selector 用来选择会话内的块：

```bash
sivtr copy codebuddy 2
sivtr copy codebuddy 2..4
sivtr copy codebuddy all --lines 1:40
```

## 过滤复制文本

过滤会在选中文本拼接完成后执行：

```bash
sivtr copy codebuddy tool --regex error --print
sivtr copy codebuddy all --lines 10:80 --print
```

需要在终端里确认结果时，加上 `--print`，不要只依赖剪贴板。

## 在 CodeBuddy Code 工作流中使用

常用一行命令：

```bash
# 在终端查看最后一条 CodeBuddy 回复。
sivtr copy codebuddy out --print

# 复制最后一个工具输出，适合查看失败的构建或测试结果。
sivtr copy codebuddy tool --print

# 从当前项目中挑选历史回答。
sivtr copy codebuddy --pick

# 直接打开当前项目的 CodeBuddy picker。
sivtr hotkey-pick-agent --cwd . --provider codebuddy
```

如果你使用 VS Code bridge，并且只想打开 CodeBuddy provider，可以把扩展参数设成：

```json
{
  "sivtr.args": ["hotkey-pick-agent", "--cwd", ".", "--provider", "codebuddy"]
}
```

## 配置额外 transcript 目录

如果 CodeBuddy transcript 树不在默认位置，可以写入配置：

```toml
[codebuddy]
session_dirs = ["/path/to/codebuddy/projects"]
```

也可以设置环境变量：

```bash
SIVTR_CODEBUDDY_SESSION_DIRS=/path/to/projects
```

Unix/macOS 多路径用 `:` 分隔，Windows 用 `;` 分隔。

## 排查问题

如果 `sivtr copy codebuddy out --print` 没有输出，或提示找不到会话：

1. 先在该项目中运行 CodeBuddy Code，确保本地 transcript 已生成。
2. 在同一个项目目录下执行命令，让 `cwd` 匹配可以找到会话。
3. 如果 transcript 存放在其他位置，配置 `[codebuddy].session_dirs` 或 `SIVTR_CODEBUDDY_SESSION_DIRS`。
4. 使用 `sivtr copy codebuddy --pick` 浏览所有可选的 CodeBuddy 会话。
