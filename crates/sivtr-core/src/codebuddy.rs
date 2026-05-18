use anyhow::Result;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use crate::ai::{
    extract_content_text, jsonl_files, normalize_path_for_match, parse_jsonl_meta,
    parse_jsonl_session, pretty_json_string, pretty_json_value, push_block, AgentBlockKind,
    AgentProvider, AgentSession, AgentSessionInfo, AgentSessionMeta, AgentSessionProvider,
};
use crate::config::SivtrConfig;

const PROVIDER_NAME: &str = "CodeBuddy";
const EXTRA_SESSION_DIRS_ENV: &str = "SIVTR_CODEBUDDY_SESSION_DIRS";

#[derive(Debug, Clone, Copy, Default)]
pub struct CodeBuddyProvider;

impl AgentSessionProvider for CodeBuddyProvider {
    fn provider(&self) -> AgentProvider {
        AgentProvider::CodeBuddy
    }

    fn list_recent_sessions(&self, cwd: Option<&Path>) -> Result<Vec<AgentSessionInfo>> {
        let wanted = cwd.map(normalize_path_for_match);
        let mut sessions = Vec::new();

        for root in configured_codebuddy_session_dirs() {
            let paths = match jsonl_files(&root) {
                Ok(paths) => paths,
                Err(error) => {
                    eprintln!(
                        "sivtr: warning: failed to read CodeBuddy session dir {}: {error:#}",
                        root.display()
                    );
                    continue;
                }
            };

            for path in paths.into_iter().filter(|path| !is_subagent_session(path)) {
                let meta = match parse_session_meta(&path) {
                    Ok(meta) => meta,
                    Err(error) => {
                        eprintln!(
                            "sivtr: warning: failed to parse CodeBuddy session metadata {}: {error:#}",
                            path.display()
                        );
                        continue;
                    }
                };

                if let Some(wanted) = wanted.as_deref() {
                    let matches_cwd = meta
                        .cwd
                        .as_deref()
                        .map(|cwd| normalize_path_for_match(Path::new(cwd)) == wanted)
                        .unwrap_or(false);
                    if !matches_cwd {
                        continue;
                    }
                }

                sessions.push(AgentSessionInfo {
                    modified: std::fs::metadata(&path)
                        .and_then(|meta| meta.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH),
                    path,
                    id: meta.id,
                    cwd: meta.cwd,
                });
            }
        }

        sessions.sort_by_key(|session| session.modified);
        sessions.reverse();
        Ok(sessions)
    }

    fn parse_session_file(&self, path: &Path) -> Result<AgentSession> {
        parse_jsonl_session(path, PROVIDER_NAME, apply_event)
    }

    fn find_session_by_id(&self, id: &str) -> Result<Option<PathBuf>> {
        let id = id.trim();
        if id.is_empty() {
            return Ok(None);
        }

        for session in self.list_recent_sessions(None)? {
            if session
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(id))
                || session
                    .id
                    .as_deref()
                    .is_some_and(|session_id| session_id == id || session_id.starts_with(id))
            {
                return Ok(Some(session.path));
            }
        }

        Ok(None)
    }

    fn find_current_session(&self, cwd: &Path) -> Result<Option<PathBuf>> {
        if let Some(path) = self.first_non_empty_session(self.list_recent_sessions(Some(cwd))?)? {
            return Ok(Some(path));
        }

        self.first_non_empty_session(self.list_recent_sessions(None)?)
    }
}

impl CodeBuddyProvider {
    fn first_non_empty_session(&self, sessions: Vec<AgentSessionInfo>) -> Result<Option<PathBuf>> {
        for session in sessions {
            match self.parse_session_file(&session.path) {
                Ok(parsed) if !parsed.blocks.is_empty() => return Ok(Some(session.path)),
                Ok(_) => {}
                Err(error) => eprintln!(
                    "sivtr: warning: failed to parse CodeBuddy session {}: {error:#}",
                    session.path.display()
                ),
            }
        }

        Ok(None)
    }
}

pub fn codebuddy_projects_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codebuddy")
        .join("projects")
}

pub fn configured_codebuddy_session_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![codebuddy_projects_dir()];

    if let Ok(config) = SivtrConfig::load() {
        dirs.extend(config.codebuddy.session_dirs);
    }

    if let Ok(extra) = std::env::var(EXTRA_SESSION_DIRS_ENV) {
        let separator = if cfg!(windows) { ';' } else { ':' };
        dirs.extend(
            extra
                .split(separator)
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(PathBuf::from),
        );
    }

    dedup_paths(dirs)
}

fn parse_session_meta(path: &Path) -> Result<AgentSessionMeta> {
    parse_jsonl_meta(path, PROVIDER_NAME, 50, update_meta)
}

fn update_meta(meta: &mut AgentSessionMeta, value: &Value) {
    if meta.id.is_none() {
        meta.id = value
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    if meta.cwd.is_none() {
        meta.cwd = value.get("cwd").and_then(Value::as_str).map(str::to_string);
    }
}

fn apply_event(session: &mut AgentSession, value: &Value) {
    update_session_meta(session, value);

    let timestamp = value
        .get("timestamp")
        .and_then(Value::as_str)
        .map(str::to_string);

    match value.get("type").and_then(Value::as_str) {
        Some("message") => apply_message(session, value, timestamp),
        Some("function_call") => push_block(
            session,
            AgentBlockKind::ToolCall,
            timestamp,
            value
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string),
            extract_tool_call_text(value),
        ),
        Some("function_call_result") => push_block(
            session,
            AgentBlockKind::ToolOutput,
            timestamp,
            None,
            extract_tool_result_text(value),
        ),
        Some("summary" | "file-history-snapshot" | "ai-title") | None => {}
        Some(_) => {}
    }
}

fn update_session_meta(session: &mut AgentSession, value: &Value) {
    if session.id.is_none() {
        session.id = value
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    if session.cwd.is_none() {
        session.cwd = value.get("cwd").and_then(Value::as_str).map(str::to_string);
    }
}

fn apply_message(session: &mut AgentSession, value: &Value, timestamp: Option<String>) {
    let kind = match value.get("role").and_then(Value::as_str) {
        Some("user") => AgentBlockKind::User,
        Some("assistant") => AgentBlockKind::Assistant,
        _ => return,
    };

    push_block(
        session,
        kind,
        timestamp,
        None,
        extract_content_text(value.get("content").unwrap_or(&Value::Null)),
    );
}

fn extract_tool_call_text(value: &Value) -> String {
    match value.get("arguments") {
        Some(Value::String(arguments)) => pretty_json_string(arguments),
        Some(arguments) => pretty_json_value(arguments),
        None => pretty_json_value(value),
    }
}

fn extract_tool_result_text(value: &Value) -> String {
    let output = value.get("output").unwrap_or(&Value::Null);
    if let Some(text) = output.get("text").and_then(Value::as_str) {
        return text.to_string();
    }

    let tool_result = value
        .get("providerData")
        .and_then(|provider_data| provider_data.get("toolResult"))
        .unwrap_or(&Value::Null);
    let content = tool_result.get("content").unwrap_or(&Value::Null);
    let content_text = extract_content_text(content);
    if !content_text.trim().is_empty() {
        return content_text;
    }

    if !output.is_null() {
        return pretty_json_value(output);
    }
    if !tool_result.is_null() {
        return pretty_json_value(tool_result);
    }

    String::new()
}

fn is_subagent_session(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => name.to_str() == Some("subagents"),
        _ => false,
    })
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for path in paths {
        if !seen.insert(normalize_path_for_match(&path)) {
            continue;
        }
        deduped.push(path);
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::{configured_codebuddy_session_dirs, CodeBuddyProvider};
    use crate::ai::{
        format_blocks, select_blocks, AgentBlockKind, AgentSelection, AgentSessionProvider,
    };
    use crate::config::SivtrConfig;
    use std::path::{Path, PathBuf};
    use std::{env, fs, time::Duration};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::test_env::lock()
    }

    fn write_session(path: &Path, session_id: &str, cwd: &Path, assistant: &str) {
        fs::write(
            path,
            format!(
                r#"{{"timestamp":"2026-05-18T00:00:00Z","type":"message","role":"user","sessionId":"{session_id}","cwd":"{}","content":[{{"input_text":"hello"}}]}}
{{"timestamp":"2026-05-18T00:00:01Z","type":"message","role":"assistant","sessionId":"{session_id}","cwd":"{}","content":[{{"output_text":"{assistant}"}}]}}
"#,
                cwd.display(),
                cwd.display()
            ),
        )
        .unwrap();
    }

    fn restore_env(name: &str, previous: Option<std::ffi::OsString>) {
        match previous {
            Some(value) => env::set_var(name, value),
            None => env::remove_var(name),
        }
    }

    #[test]
    fn parses_codebuddy_messages_and_tools() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(
            &path,
            r#"{"timestamp":"2026-05-18T00:00:00Z","type":"message","role":"user","sessionId":"cb-session","cwd":"/repo","content":[{"input_text":"hello"}]}
{"timestamp":"2026-05-18T00:00:01Z","type":"function_call","sessionId":"cb-session","cwd":"/repo","name":"Bash","arguments":"{\"command\":\"cargo test\"}"}
{"timestamp":"2026-05-18T00:00:02Z","type":"function_call_result","sessionId":"cb-session","cwd":"/repo","output":{"text":"tests ok"}}
{"timestamp":"2026-05-18T00:00:03Z","type":"message","role":"assistant","sessionId":"cb-session","cwd":"/repo","content":[{"output_text":"done"}]}
"#,
        )
        .unwrap();

        let session = CodeBuddyProvider.parse_session_file(&path).unwrap();

        assert_eq!(session.id.as_deref(), Some("cb-session"));
        assert_eq!(session.cwd.as_deref(), Some("/repo"));
        assert_eq!(session.blocks.len(), 4);
        assert_eq!(session.blocks[0].kind, AgentBlockKind::User);
        assert_eq!(session.blocks[1].kind, AgentBlockKind::ToolCall);
        assert_eq!(session.blocks[1].label.as_deref(), Some("Bash"));
        assert!(session.blocks[1].text.contains("cargo test"));
        assert_eq!(session.blocks[2].kind, AgentBlockKind::ToolOutput);
        assert_eq!(session.blocks[2].text, "tests ok");
        assert_eq!(session.blocks[3].kind, AgentBlockKind::Assistant);
    }

    #[test]
    fn skips_empty_internal_and_unknown_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(
            &path,
            r#"{"type":"summary","sessionId":"cb-session","cwd":"/repo","content":"ignore"}
{"type":"file-history-snapshot","sessionId":"cb-session","cwd":"/repo","content":"ignore"}
{"type":"ai-title","sessionId":"cb-session","cwd":"/repo","content":"ignore"}
{"type":"future-event","sessionId":"cb-session","cwd":"/repo","content":"ignore"}
{"type":"message","role":"assistant","sessionId":"cb-session","cwd":"/repo","content":[]}
{"type":"message","role":"assistant","sessionId":"cb-session","cwd":"/repo","content":[{"output_text":"visible"}]}
"#,
        )
        .unwrap();

        let session = CodeBuddyProvider.parse_session_file(&path).unwrap();

        assert_eq!(session.blocks.len(), 1);
        assert_eq!(session.blocks[0].kind, AgentBlockKind::Assistant);
        assert_eq!(session.blocks[0].text, "visible");
    }

    #[test]
    fn extracts_tool_result_by_priority() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(
            &path,
            r#"{"type":"function_call_result","output":{"text":"from output"}}
{"type":"function_call_result","providerData":{"toolResult":{"content":[{"text":"from provider"}]}}}
{"type":"function_call_result","output":{"value":1}}
{"type":"function_call_result","providerData":{"toolResult":{"value":2}}}
"#,
        )
        .unwrap();

        let session = CodeBuddyProvider.parse_session_file(&path).unwrap();

        assert_eq!(session.blocks.len(), 4);
        assert_eq!(session.blocks[0].text, "from output");
        assert_eq!(session.blocks[1].text, "from provider");
        assert!(session.blocks[2].text.contains("\"value\": 1"));
        assert!(session.blocks[3].text.contains("\"value\": 2"));
    }

    #[test]
    fn formats_valid_tool_arguments_and_keeps_invalid_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(
            &path,
            r#"{"type":"function_call","name":"good","arguments":"{\"alpha\":1}"}
{"type":"function_call","name":"bad","arguments":"not json"}
"#,
        )
        .unwrap();

        let session = CodeBuddyProvider.parse_session_file(&path).unwrap();

        assert_eq!(session.blocks.len(), 2);
        assert!(session.blocks[0].text.contains("\"alpha\": 1"));
        assert_eq!(session.blocks[1].text, "not json");
    }

    #[test]
    fn tolerates_trailing_partial_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(
            &path,
            r#"{"type":"message","role":"assistant","sessionId":"cb-session","cwd":"/repo","content":[{"output_text":"done"}]}
{"type":"message","role":"assistant""#,
        )
        .unwrap();

        let session = CodeBuddyProvider.parse_session_file(&path).unwrap();

        assert_eq!(session.blocks.len(), 1);
        assert_eq!(session.blocks[0].text, "done");
    }

    #[test]
    fn select_modes_do_not_mix_assistant_and_tool_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(
            &path,
            r#"{"type":"message","role":"user","content":[{"input_text":"question"}]}
{"type":"function_call_result","output":{"text":"tool output"}}
{"type":"message","role":"assistant","content":[{"output_text":"assistant answer"}]}
"#,
        )
        .unwrap();

        let session = CodeBuddyProvider.parse_session_file(&path).unwrap();
        let assistant = format_blocks(&select_blocks(&session, AgentSelection::LastAssistant));
        let tool = format_blocks(&select_blocks(&session, AgentSelection::LastTool));
        let all = format_blocks(&select_blocks(&session, AgentSelection::All));

        assert_eq!(assistant, "assistant answer");
        assert_eq!(tool, "tool output");
        assert!(all.contains("assistant answer"));
        assert!(all.contains("tool output"));
    }

    #[test]
    fn list_sessions_filters_subagents_and_sorts_by_modified_time() {
        let _guard = env_lock();
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let projects = home.join(".codebuddy").join("projects");
        let main_dir = projects.join("project").join("main");
        let subagent_dir = projects.join("project").join("main").join("subagents");
        fs::create_dir_all(&main_dir).unwrap();
        fs::create_dir_all(&subagent_dir).unwrap();
        let old = main_dir.join("old.jsonl");
        let new = main_dir.join("new.jsonl");
        let subagent = subagent_dir.join("sub.jsonl");
        let cwd = temp.path().join("repo");
        write_session(&old, "old", &cwd, "old answer");
        std::thread::sleep(Duration::from_millis(20));
        write_session(&new, "new", &cwd, "new answer");
        write_session(&subagent, "sub", &cwd, "sub answer");

        let previous_home = env::var_os("HOME");
        let previous_userprofile = env::var_os("USERPROFILE");
        let previous_extra = env::var_os("SIVTR_CODEBUDDY_SESSION_DIRS");
        env::set_var("HOME", &home);
        env::set_var("USERPROFILE", &home);
        env::remove_var("SIVTR_CODEBUDDY_SESSION_DIRS");

        let sessions = CodeBuddyProvider.list_recent_sessions(None).unwrap();

        restore_env("HOME", previous_home);
        restore_env("USERPROFILE", previous_userprofile);
        restore_env("SIVTR_CODEBUDDY_SESSION_DIRS", previous_extra);

        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].id.as_deref(), Some("new"));
        assert_eq!(sessions[1].id.as_deref(), Some("old"));
        assert!(!sessions
            .iter()
            .any(|session| session.id.as_deref() == Some("sub")));
    }

    #[test]
    fn find_current_session_prefers_cwd_match_and_falls_back_to_latest_global() {
        let _guard = env_lock();
        let temp = tempfile::tempdir().unwrap();
        let projects = temp.path().join("projects");
        fs::create_dir_all(&projects).unwrap();
        let matching_cwd = temp.path().join("matching");
        let other_cwd = temp.path().join("other");
        let matching = projects.join("matching.jsonl");
        let latest = projects.join("latest.jsonl");
        write_session(&matching, "matching", &matching_cwd, "matching answer");
        std::thread::sleep(Duration::from_millis(20));
        write_session(&latest, "latest", &other_cwd, "latest answer");

        let previous_home = env::var_os("HOME");
        let previous_userprofile = env::var_os("USERPROFILE");
        let previous_extra = env::var_os("SIVTR_CODEBUDDY_SESSION_DIRS");
        let home = temp.path().join("home");
        env::set_var("HOME", &home);
        env::set_var("USERPROFILE", &home);
        env::set_var("SIVTR_CODEBUDDY_SESSION_DIRS", &projects);

        let matched = CodeBuddyProvider
            .find_current_session(&matching_cwd)
            .unwrap();
        let fallback = CodeBuddyProvider
            .find_current_session(&temp.path().join("missing"))
            .unwrap();

        restore_env("HOME", previous_home);
        restore_env("USERPROFILE", previous_userprofile);
        restore_env("SIVTR_CODEBUDDY_SESSION_DIRS", previous_extra);

        assert_eq!(matched.as_deref(), Some(matching.as_path()));
        assert_eq!(fallback.as_deref(), Some(latest.as_path()));
    }

    #[test]
    fn find_session_by_id_accepts_prefix() {
        let _guard = env_lock();
        let temp = tempfile::tempdir().unwrap();
        let projects = temp.path().join("projects");
        fs::create_dir_all(&projects).unwrap();
        let cwd = temp.path().join("repo");
        let path = projects.join("session.jsonl");
        write_session(&path, "codebuddy-session-123", &cwd, "answer");

        let previous_extra = env::var_os("SIVTR_CODEBUDDY_SESSION_DIRS");
        env::set_var("SIVTR_CODEBUDDY_SESSION_DIRS", &projects);

        let resolved = CodeBuddyProvider
            .find_session_by_id("codebuddy-session")
            .unwrap();

        restore_env("SIVTR_CODEBUDDY_SESSION_DIRS", previous_extra);

        assert_eq!(resolved.as_deref(), Some(path.as_path()));
    }

    #[test]
    fn configured_session_dirs_combines_default_config_and_env_entries() {
        let _guard = env_lock();
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let config_home = temp.path().join("config-home");
        let previous_home = env::var_os("HOME");
        let previous_userprofile = env::var_os("USERPROFILE");
        let previous_xdg_config_home = env::var_os("XDG_CONFIG_HOME");
        let previous_appdata = env::var_os("APPDATA");
        let previous_extra = env::var_os("SIVTR_CODEBUDDY_SESSION_DIRS");
        let separator = if cfg!(windows) { ";" } else { ":" };
        env::set_var("HOME", &home);
        env::set_var("USERPROFILE", &home);
        env::set_var("XDG_CONFIG_HOME", &config_home);
        if cfg!(windows) {
            env::set_var("APPDATA", &config_home);
        }

        let config_path = SivtrConfig::config_path().unwrap();
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(
            &config_path,
            "[codebuddy]\nsession_dirs = [\"/tmp/from-config\", \"/tmp/shared\"]\n",
        )
        .unwrap();
        env::set_var(
            "SIVTR_CODEBUDDY_SESSION_DIRS",
            format!("/tmp/from-env{separator}/tmp/shared"),
        );

        let dirs = configured_codebuddy_session_dirs();

        restore_env("HOME", previous_home);
        restore_env("USERPROFILE", previous_userprofile);
        restore_env("XDG_CONFIG_HOME", previous_xdg_config_home);
        restore_env("APPDATA", previous_appdata);
        restore_env("SIVTR_CODEBUDDY_SESSION_DIRS", previous_extra);

        assert!(contains_path(
            &dirs,
            home.join(".codebuddy").join("projects")
        ));
        assert!(contains_path(&dirs, PathBuf::from("/tmp/from-config")));
        assert!(contains_path(&dirs, PathBuf::from("/tmp/from-env")));
        assert_eq!(count_path(&dirs, PathBuf::from("/tmp/shared")), 1);
    }

    fn contains_path(dirs: &[PathBuf], expected: PathBuf) -> bool {
        dirs.iter().any(|path| paths_match(path, &expected))
    }

    fn count_path(dirs: &[PathBuf], expected: PathBuf) -> usize {
        dirs.iter()
            .filter(|path| paths_match(path, &expected))
            .count()
    }

    fn paths_match(left: &Path, right: &Path) -> bool {
        left == right || left.to_string_lossy() == right.to_string_lossy()
    }
}
