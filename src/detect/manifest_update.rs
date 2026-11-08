use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use super::{agent_label, parse_agent_label, Agent};

pub(crate) const MANIFEST_ENGINE_VERSION: u32 = 3;
/// The catalog is a signed asset of the latest release: `release.yml` signs it
/// (and every agent manifest it lists) with the release key.
const CATALOG_URL: &str =
    "https://github.com/vjk7989/h0x-ADE-Runtime/releases/latest/download/agent-detection.toml";
const MAX_FETCH_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct ManifestVersion(String);

impl ManifestVersion {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("version must not be empty".to_string());
        }
        for segment in trimmed.split('.') {
            if segment.is_empty() {
                return Err(format!("version {trimmed:?} contains an empty segment"));
            }
            if !segment.chars().all(|ch| ch.is_ascii_digit()) {
                return Err(format!("version {trimmed:?} must be dotted numeric"));
            }
            segment
                .parse::<u64>()
                .map_err(|_| format!("version {trimmed:?} contains an oversized segment"))?;
        }
        Ok(Self(trimmed.to_string()))
    }
}

impl fmt::Display for ManifestVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ManifestVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ManifestVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl Ord for ManifestVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut left = self.0.split('.');
        let mut right = other.0.split('.');

        loop {
            match (left.next(), right.next()) {
                (Some(left), Some(right)) => {
                    let left = left.parse::<u64>().unwrap_or(0);
                    let right = right.parse::<u64>().unwrap_or(0);
                    match left.cmp(&right) {
                        Ordering::Equal => {}
                        ordering => return ordering,
                    }
                }
                (Some(left), None) => {
                    let left = left.parse::<u64>().unwrap_or(0);
                    if left == 0 {
                        continue;
                    }
                    return Ordering::Greater;
                }
                (None, Some(right)) => {
                    let right = right.parse::<u64>().unwrap_or(0);
                    if right == 0 {
                        continue;
                    }
                    return Ordering::Less;
                }
                (None, None) => return Ordering::Equal,
            }
        }
    }
}

impl PartialOrd for ManifestVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ManifestVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ManifestVersion {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestUpdateCommit {
    pub(crate) agent: Agent,
    pub(crate) version: ManifestVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) struct ManifestUpdateStatus {
    pub(crate) last_check_unix: Option<u64>,
    pub(crate) last_result: Option<String>,
    #[serde(default)]
    pub(crate) agents: BTreeMap<String, AgentRemoteStatus>,
}

impl ManifestUpdateStatus {
    pub(crate) fn agent_status(&self, agent: Agent) -> Option<AgentRemoteStatus> {
        self.agents.get(agent_label(agent)).cloned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentRemoteStatus {
    pub(crate) cached_version: Option<String>,
    pub(crate) attempted_version: Option<String>,
    pub(crate) last_checked_unix: Option<u64>,
    pub(crate) last_result: String,
    pub(crate) last_error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestCatalog {
    schema_version: u32,
    #[serde(default)]
    agents: Vec<ManifestCatalogAgent>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestCatalogAgent {
    id: String,
    path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CatalogAgent {
    agent: Agent,
    path: String,
}

pub(crate) fn auto_update(events: tokio::sync::mpsc::Sender<crate::events::AppEvent>) {
    auto_update_from(events, CATALOG_URL, crate::update::RELEASE_PUBLIC_KEY);
}

fn auto_update_from(
    events: tokio::sync::mpsc::Sender<crate::events::AppEvent>,
    url: &str,
    key: &str,
) {
    let result = check_and_update_from_url(url, key);
    let status = match result {
        Ok(output) => {
            let activated = agents_needing_cache_reload(&output);
            if !activated.is_empty() {
                super::manifest::reload_manifests_for_agents(&activated);
            }
            let _ = events.blocking_send(crate::events::AppEvent::AgentDetectionManifestsUpdated {
                updated: output.updated,
                activated,
                status: output.status,
            });
            return;
        }
        Err(err) => {
            tracing::warn!("agent detection manifest update failed: {err}");
            let mut status = load_status();
            status.last_check_unix = Some(now_unix());
            status.last_result = Some(format!("failed: {err}"));
            let _ = save_status(&status);
            status
        }
    };
    let _ = events.blocking_send(crate::events::AppEvent::AgentDetectionManifestsUpdated {
        updated: Vec::new(),
        activated: Vec::new(),
        status,
    });
}

fn agents_needing_cache_reload(output: &ManifestUpdateOutput) -> Vec<Agent> {
    let loaded = super::manifest::manifest_summaries();
    let mut activated = output
        .updated
        .iter()
        .map(|commit| commit.agent)
        .collect::<Vec<_>>();

    for agent in &output.checked {
        let agent = *agent;
        let Some(status) = output.status.agent_status(agent) else {
            continue;
        };
        if status.last_result != "current" && status.last_result != "updated" {
            continue;
        }
        let loaded_version = loaded
            .iter()
            .find(|summary| summary.agent == agent)
            .and_then(|summary| summary.cached_remote_version.as_deref());
        let disk_version = cached_remote_version(agent).map(|version| version.to_string());
        if loaded_version != disk_version.as_deref() && !activated.contains(&agent) {
            activated.push(agent);
        }
    }

    activated
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestUpdateOutput {
    pub(crate) checked: Vec<Agent>,
    pub(crate) updated: Vec<ManifestUpdateCommit>,
    pub(crate) status: ManifestUpdateStatus,
}

pub(crate) fn check_and_update() -> Result<ManifestUpdateOutput, String> {
    check_and_update_from_url(CATALOG_URL, crate::update::RELEASE_PUBLIC_KEY)
}

fn check_and_update_from_url(url: &str, key: &str) -> Result<ManifestUpdateOutput, String> {
    let catalog = parse_catalog(&fetch_signed_text(url, key)?)?;
    let base_url = base_url(url)?;
    let mut status = load_status();
    let check_time = now_unix();
    status.last_check_unix = Some(check_time);
    status.last_result = Some("checked".to_string());

    let checked = catalog.iter().map(|entry| entry.agent).collect::<Vec<_>>();
    let mut updated = Vec::new();
    for entry in catalog {
        let agent_id = agent_label(entry.agent).to_string();
        let manifest_url = join_url(&base_url, &entry.path)?;
        match fetch_signed_text(&manifest_url, key)
            .map_err(|err| format!("fetch failed: {err}"))
            .and_then(|content| process_agent_manifest(entry.agent, &content, check_time))
        {
            Ok(Some(commit)) => {
                status.agents.insert(
                    agent_id,
                    AgentRemoteStatus {
                        cached_version: Some(commit.version.to_string()),
                        attempted_version: Some(commit.version.to_string()),
                        last_checked_unix: Some(check_time),
                        last_result: "updated".to_string(),
                        last_error: None,
                    },
                );
                updated.push(commit);
            }
            Ok(None) => {
                let cached_version = cached_remote_version(entry.agent);
                status.agents.insert(
                    agent_id,
                    AgentRemoteStatus {
                        cached_version: cached_version.map(|version| version.to_string()),
                        attempted_version: None,
                        last_checked_unix: Some(check_time),
                        last_result: "current".to_string(),
                        last_error: None,
                    },
                );
            }
            Err(err) => {
                tracing::warn!(
                    agent = agent_label(entry.agent),
                    error = %err,
                    "agent detection manifest update failed for agent"
                );
                let cached_version = cached_remote_version(entry.agent);
                status.agents.insert(
                    agent_id,
                    AgentRemoteStatus {
                        cached_version: cached_version.map(|version| version.to_string()),
                        attempted_version: None,
                        last_checked_unix: Some(check_time),
                        last_result: "failed".to_string(),
                        last_error: Some(err),
                    },
                );
            }
        }
    }

    if let Err(err) = save_status(&status) {
        tracing::warn!("failed to save agent detection manifest update status: {err}");
        status.last_result = Some(format!("failed_to_save_status: {err}"));
    }
    Ok(ManifestUpdateOutput {
        checked,
        updated,
        status,
    })
}

fn process_agent_manifest(
    agent: Agent,
    content: &str,
    _check_time: u64,
) -> Result<Option<ManifestUpdateCommit>, String> {
    let parsed = super::manifest::parse_remote_manifest_for_agent(agent, content)?;
    if let Some(current) = cached_remote_version(agent) {
        match parsed.version.cmp(&current) {
            Ordering::Less => {
                return Err(format!(
                    "remote version {} is older than cached {current}",
                    parsed.version
                ))
            }
            Ordering::Equal => {
                let committed = fs::read_to_string(remote_manifest_path(agent)).unwrap_or_default();
                if committed != content {
                    return Err(format!(
                        "remote version {} changed content without a version bump",
                        parsed.version
                    ));
                }
                return Ok(None);
            }
            Ordering::Greater => {}
        }
    }

    commit_remote_manifest(agent, content)?;
    Ok(Some(ManifestUpdateCommit {
        agent,
        version: parsed.version,
    }))
}

fn parse_catalog(content: &str) -> Result<Vec<CatalogAgent>, String> {
    let catalog: ManifestCatalog =
        toml::from_str(content).map_err(|err| format!("failed to parse catalog TOML: {err}"))?;
    if catalog.schema_version != 1 {
        return Err(format!(
            "unsupported catalog schema_version {}",
            catalog.schema_version
        ));
    }

    let mut seen = BTreeSet::new();
    let mut agents = Vec::new();
    for entry in catalog.agents {
        let Some(agent) = parse_agent_label(&entry.id) else {
            tracing::warn!(agent = entry.id, "skipping unknown remote manifest agent");
            continue;
        };
        if entry.path.trim().is_empty() {
            return Err(format!("catalog entry {} has an empty path", entry.id));
        }
        if entry.path.contains("://")
            || entry.path.starts_with('/')
            || entry.path.split('/').any(|part| part == "..")
        {
            return Err(format!(
                "catalog entry {} has an unsafe path {}",
                entry.id, entry.path
            ));
        }
        if !seen.insert(agent_label(agent).to_string()) {
            return Err(format!("catalog contains duplicate agent {}", entry.id));
        }
        agents.push(CatalogAgent {
            agent,
            path: entry.path,
        });
    }
    Ok(agents)
}

pub(crate) fn load_status() -> ManifestUpdateStatus {
    let path = status_path();
    let Ok(content) = fs::read_to_string(&path) else {
        return ManifestUpdateStatus::default();
    };
    toml::from_str(&content).unwrap_or_else(|err| {
        tracing::warn!(
            path = %path.display(),
            "failed to parse agent detection manifest status: {err}"
        );
        ManifestUpdateStatus::default()
    })
}

fn save_status(status: &ManifestUpdateStatus) -> Result<(), String> {
    let path = status_path();
    let parent = path
        .parent()
        .ok_or_else(|| format!("status path {} has no parent", path.display()))?;
    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    let content = toml::to_string_pretty(status).map_err(|err| err.to_string())?;
    atomic_write(&path, content.as_bytes())
}

pub(crate) fn status_path() -> PathBuf {
    state_root().join("status.toml")
}

pub(crate) fn remote_manifest_path(agent: Agent) -> PathBuf {
    state_root()
        .join("remote")
        .join(format!("{}.toml", agent_label(agent)))
}

pub(crate) fn cached_remote_version(agent: Agent) -> Option<ManifestVersion> {
    let content = fs::read_to_string(remote_manifest_path(agent)).ok()?;
    super::manifest::parse_remote_manifest_for_agent(agent, &content)
        .ok()
        .map(|parsed| parsed.version)
}

fn commit_remote_manifest(agent: Agent, content: &str) -> Result<(), String> {
    let path = remote_manifest_path(agent);
    let parent = path
        .parent()
        .ok_or_else(|| format!("remote manifest path {} has no parent", path.display()))?;
    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    atomic_write(&path, content.as_bytes())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("path {} has no parent", path.display()))?;
    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    let tmp_path = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("manifest"),
        std::process::id(),
        now_nanos()
    ));
    {
        let mut file = fs::File::create(&tmp_path).map_err(|err| err.to_string())?;
        if let Err(err) = file.write_all(bytes).and_then(|_| file.sync_all()) {
            let _ = fs::remove_file(&tmp_path);
            return Err(err.to_string());
        }
    }
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        err.to_string()
    })?;
    if let Err(err) = sync_parent_dir(parent) {
        tracing::warn!(
            path = %path.display(),
            error = %err,
            "agent detection manifest committed but parent directory sync failed"
        );
    }
    Ok(())
}

fn sync_parent_dir(parent: &Path) -> Result<(), String> {
    let dir = match fs::File::open(parent) {
        Ok(dir) => dir,
        Err(err) if directory_sync_unsupported(&err) => return Ok(()),
        Err(err) => return Err(format!("failed to open parent directory for sync: {err}")),
    };
    match dir.sync_all() {
        Ok(()) => Ok(()),
        Err(err) if directory_sync_unsupported(&err) => Ok(()),
        Err(err) => Err(format!("failed to sync parent directory: {err}")),
    }
}

fn directory_sync_unsupported(err: &std::io::Error) -> bool {
    // PermissionDenied: Windows cannot open directories via `File::open`
    // (os error 5), so directory sync is effectively unsupported there.
    matches!(
        err.kind(),
        std::io::ErrorKind::Unsupported
            | std::io::ErrorKind::InvalidInput
            | std::io::ErrorKind::PermissionDenied
    )
}

fn state_root() -> PathBuf {
    crate::config::state_dir().join("agent-detection")
}

/// Fetches `url` and returns it only when `<url>.minisig` is a valid signature
/// by `key` made for this file name; anything else is an error and the caller
/// keeps what it had.
fn fetch_signed_text(url: &str, key: &str) -> Result<String, String> {
    let text = fetch_text(url)?;
    crate::update::verify_release_signature(
        url,
        url.rsplit('/').next().unwrap_or(url),
        text.as_bytes(),
        crate::update::fetch_release_signature(url).as_deref(),
        key,
    )?;
    Ok(text)
}

fn fetch_text(url: &str) -> Result<String, String> {
    let max_fetch_bytes = MAX_FETCH_BYTES.to_string();
    let mut child = crate::noninteractive_process::curl_command()
        .args([
            "-sfL",
            "--retry",
            "2",
            "--connect-timeout",
            "5",
            "--max-time",
            "15",
            "--max-filesize",
            &max_fetch_bytes,
            url,
        ])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|err| format!("curl failed: {err}"))?;

    let mut bytes = Vec::new();
    let Some(stdout) = child.stdout.as_mut() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("curl stdout was not captured".to_string());
    };
    stdout
        .take((MAX_FETCH_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|err| {
            let _ = child.kill();
            let _ = child.wait();
            format!("failed to read curl response: {err}")
        })?;
    if bytes.len() > MAX_FETCH_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "response from {url} exceeded {MAX_FETCH_BYTES} bytes"
        ));
    }

    let status = child
        .wait()
        .map_err(|err| format!("curl wait failed: {err}"))?;
    if !status.success() {
        return Err(format!("failed to fetch {url}"));
    }
    String::from_utf8(bytes).map_err(|err| format!("response was not UTF-8: {err}"))
}

fn base_url(url: &str) -> Result<String, String> {
    let Some((base, _)) = url.rsplit_once('/') else {
        return Err(format!("catalog URL {url} has no base path"));
    };
    Ok(base.to_string())
}

fn join_url(base: &str, path: &str) -> Result<String, String> {
    if path.contains("://") || path.starts_with('/') || path.split('/').any(|part| part == "..") {
        return Err(format!("unsafe manifest path {path}"));
    }
    Ok(format!("{}/{}", base.trim_end_matches('/'), path))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn remote_manifest(version: &str, contains: &str) -> String {
        remote_manifest_for("codex", version, contains)
    }

    fn remote_manifest_for(agent: &str, version: &str, contains: &str) -> String {
        format!(
            r#"
id = "{agent}"
version = "{version}"
min_engine_version = 1
updated_at = "2026-06-10T12:00:00Z"

[[rules]]
id = "idle"
state = "idle"
contains = ["{contains}"]
"#
        )
    }

    fn with_state_dir<T>(name: &str, f: impl FnOnce() -> T) -> T {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let old_config = std::env::var_os("XDG_CONFIG_HOME");
        let old_state = std::env::var_os("XDG_STATE_HOME");
        let dir =
            std::env::temp_dir().join(format!("h0x-manifest-update-{name}-{}", std::process::id()));
        let config_dir = dir.join("config");
        let state_dir = dir.join("state");
        let _ = fs::remove_dir_all(&dir);
        std::env::set_var("XDG_CONFIG_HOME", &config_dir);
        std::env::set_var("XDG_STATE_HOME", &state_dir);
        crate::detect::manifest::reload_manifests();
        let result = f();
        match old_config {
            Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match old_state {
            Some(value) => std::env::set_var("XDG_STATE_HOME", value),
            None => std::env::remove_var("XDG_STATE_HOME"),
        }
        crate::detect::manifest::reload_manifests();
        let _ = fs::remove_dir_all(&dir);
        result
    }

    /// A throwaway key pair (not the release key); the signatures below are
    /// made with it by `rsign sign -W -t "timestamp:1<TAB>file:<name>"` over the
    /// exact bytes the tests write. `SIG_*_OTHER_KEY` is from a second throwaway key.
    const TEST_KEY: &str = "RWRoiCZgYc65YANz+u2IxSwI5GlIK0sCuai/aFXAhHbjOk4aFQJYQ0kR";

    fn file_url(web_dir: &Path) -> String {
        format!(
            "file:///{}",
            web_dir
                .join("index.toml")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches('/')
        )
    }

    fn put_sig(web_dir: &Path, name: &str, sig: &str) {
        fs::write(web_dir.join(format!("{name}.minisig")), sig).unwrap();
    }

    fn web_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("h0x-manifest-sig-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Serves a catalog with the given index and codex manifest (each with an
    /// optional signature), with codex 9999.01.01.1 already cached, and runs one
    /// update against `key`.
    fn serve_and_update(
        name: &str,
        index: (&str, Option<&str>),
        codex: (&str, Option<&str>),
        key: &str,
    ) -> Result<ManifestUpdateOutput, String> {
        let dir = web_dir(name);
        fs::write(dir.join("index.toml"), index.0).unwrap();
        if let Some(sig) = index.1 {
            put_sig(&dir, "index.toml", sig);
        }
        fs::write(dir.join("codex.toml"), codex.0).unwrap();
        if let Some(sig) = codex.1 {
            put_sig(&dir, "codex.toml", sig);
        }
        process_agent_manifest(
            Agent::Codex,
            &remote_manifest("9999.01.01.1", "initial-ready"),
            1,
        )
        .unwrap();
        let result = check_and_update_from_url(&file_url(&dir), key);
        let _ = fs::remove_dir_all(&dir);
        result
    }

    const INDEX_A: &str = r#"
schema_version = 1

[[agents]]
id = "codex"
path = "codex.toml"
"#;

    fn cached_codex_version() -> Option<String> {
        cached_remote_version(Agent::Codex).map(|v| v.to_string())
    }

    #[test]
    fn catalog_is_a_release_asset_not_the_main_branch() {
        assert_eq!(
            CATALOG_URL,
            "https://github.com/vjk7989/h0x-ADE-Runtime/releases/latest/download/agent-detection.toml"
        );
    }

    #[test]
    fn signed_catalog_and_manifest_are_applied() {
        with_state_dir("sig-good", || {
            let manifest = remote_manifest("9999.01.01.2", "current-ready");
            let output = serve_and_update(
                "good",
                (INDEX_A, Some(SIG_IDX_A)),
                (&manifest, Some(SIG_M_CURRENT)),
                TEST_KEY,
            )
            .unwrap();
            assert_eq!(output.updated.len(), 1);
            assert_eq!(cached_codex_version().as_deref(), Some("9999.01.01.2"));
        });
    }

    #[test]
    fn unsigned_or_wrongly_signed_manifest_keeps_the_cached_one() {
        let manifest = remote_manifest("9999.01.01.2", "current-ready");
        let tampered = format!("{manifest}# tampered\n");
        let cases: [(&str, &str, Option<&str>); 4] = [
            ("unsigned", &manifest, None),
            ("other-file", &manifest, Some(SIG_M_CURRENT_OTHER_FILE)),
            ("other-key", &manifest, Some(SIG_M_CURRENT_OTHER_KEY)),
            ("tampered", &tampered, Some(SIG_M_CURRENT)),
        ];
        for (name, content, sig) in cases {
            with_state_dir(&format!("sig-bad-{name}"), || {
                let output =
                    serve_and_update(name, (INDEX_A, Some(SIG_IDX_A)), (content, sig), TEST_KEY)
                        .unwrap();
                assert!(output.updated.is_empty(), "{name}: applied");
                assert_eq!(
                    cached_codex_version().as_deref(),
                    Some("9999.01.01.1"),
                    "{name}: cached manifest changed"
                );
                let status = output.status.agents.get("codex").unwrap();
                assert_eq!(status.last_result, "failed", "{name}");
                assert!(
                    status
                        .last_error
                        .as_deref()
                        .is_some_and(|e| e.contains("not signed by the h0x release key")),
                    "{name}: {status:?}"
                );
            });
        }
    }

    #[test]
    fn unsigned_or_wrongly_signed_catalog_is_refused() {
        let manifest = remote_manifest("9999.01.01.2", "current-ready");
        let tampered = format!("{INDEX_A}# tampered\n");
        let cases: [(&str, &str, Option<&str>, &str); 5] = [
            ("unsigned", INDEX_A, None, TEST_KEY),
            ("tampered", &tampered, Some(SIG_IDX_A), TEST_KEY),
            ("other-file", INDEX_A, Some(SIG_M_CURRENT), TEST_KEY),
            // Signed by the test key, but the runtime trusts the release key.
            (
                "release-key",
                INDEX_A,
                Some(SIG_IDX_A),
                crate::update::RELEASE_PUBLIC_KEY,
            ),
            ("garbage-sig", INDEX_A, Some("not a signature"), TEST_KEY),
        ];
        for (name, index, sig, key) in cases {
            with_state_dir(&format!("sig-bad-index-{name}"), || {
                let result =
                    serve_and_update(name, (index, sig), (&manifest, Some(SIG_M_CURRENT)), key);
                let err = result.expect_err(name);
                assert!(
                    err.contains("not signed by the h0x release key"),
                    "{name}: {err}"
                );
                assert_eq!(
                    cached_codex_version().as_deref(),
                    Some("9999.01.01.1"),
                    "{name}"
                );
            });
        }
    }

    #[test]
    fn unreachable_catalog_keeps_the_cached_manifest() {
        with_state_dir("sig-offline", || {
            process_agent_manifest(
                Agent::Codex,
                &remote_manifest("9999.01.01.1", "initial-ready"),
                1,
            )
            .unwrap();
            let dir = web_dir("offline");
            let result = check_and_update_from_url(&file_url(&dir), TEST_KEY);
            let _ = fs::remove_dir_all(&dir);
            assert!(result.is_err());
            assert_eq!(cached_codex_version().as_deref(), Some("9999.01.01.1"));
        });
    }

    const SIG_IDX_A: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YEhfHfjLZ6FVW48oF8iEOMnbXTA7bXrtYO+a8A+VkVtsdW7ipKuKpRN9qarXImmaIdP8jdT9fAxjxWmDBYMccQc=
trusted comment: timestamp:1\tfile:index.toml
JQNLK0lCCjzRgoYWX25VgQh4l+h5Vfo1qSYW59TTycB6VMh54dFnRukAoEHqeCbh4ZS9jAYuEmyNPuVxDXJeBQ==
";
    const SIG_IDX_B: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YJLRQX7aXOhwkR3WsVjZ4vuYXyu7TBj9g7UOqgP2wUCX7VFs+OcwiRMDBkrp/8VFrLXs9wLXzvif98lsm8qGkw0=
trusted comment: timestamp:1\tfile:index.toml
Bdq5DLXZN3PCEa+L9uPmbnX4SfGh3ilz3Ufmp+6KLbgpQdArniE2pguIbRDx+3q+CLqE2NyHrRBhk3inQIl+Cw==
";
    const SIG_M_READY: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YB6DHga+ln6M/zbJ+Nmad9TJ0gHWqKXFSsslar4uJ4TD0vZP8m7udtgHemLXRCJ63tjTWj8HOhMKkoOZwXy4rws=
trusted comment: timestamp:1\tfile:codex.toml
MSZOYJzTmi+rl95XgP2cW7F06Ky2I4IAq03/f69RSy3Jz7fc6ftgTNj2j4nsA3RPXAKY3OCt61rrDhm6pDAiBQ==
";
    const SIG_M_CURRENT: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YOonO5mlKw94AdaYWcUkZD9pNxLimEBSlP1v1Tk4S4KegwZ1ZSs9z43yquTMn44429/UNdhnLpavPh0iK1W/SAY=
trusted comment: timestamp:1\tfile:codex.toml
4AKE1VyZY1as2QPCNs/MfqHy8D+pa0ZWl2uTHFGYeChJuqpWHeLTAe+2DBFqkIHilQCijnaZnhAkbfZuafT6Bw==
";
    const SIG_M_CODEX_CURRENT: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YFBsnYsDTUqyyyCYPAAJlr7Ro4lrOMqQ8AlKc+AIWiZhT3lcCbyx5VKbWJFSor9X0t3uuDyJIpO39ZVcmdFl6wI=
trusted comment: timestamp:1\tfile:codex.toml
EsSgHusY7KNCUeQBWMnY4FGLEzB8aLN4cFRx7UkIDZ/Pm0Aib0gUDPZE83nnGh2HWLrzFK9KzSBAAHeteuMDCg==
";
    const SIG_M_CURRENT_OTHER_FILE: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YD73B3ZbtUekFLC95MgboZAe1TaQMIjZcoZ9tQl0LBcGin2mA+ZvrpoY/37ZgEp8ytWC13KCH2hoyUSUcTqc+AI=
trusted comment: timestamp:1\tfile:cursor.toml
wCqEqSpLtVnNWaPg5OnyrV9JPfdvDSOL9CUU+F/0694+aO02VkvBUtRpHhw+OB9HnFHQnYWzQKaz03vi/E46Cw==
";
    const SIG_M_CURRENT_OTHER_KEY: &str = "untrusted comment: signature from rsign secret key
RURoaNYB1lrhrPjIMgHflYAQS4O6FoEWaLewWM8+jUjfmpgwfcexW1vUkEc/EtNwhtmkBg+7xJdFT3zR/Y0E7qowWQIXRBkNSwg=
trusted comment: timestamp:1\tfile:codex.toml
NEZO1V79BoDa0Izdjv/v9nUfrclBSwC2leSP3xVnYS9zmhhUDi7c7mHkmQe9MfiIt5gf3FgqvnmbKha1Lr7RAQ==
";

    #[test]
    fn manifest_version_compares_dotted_numeric_segments() {
        assert!(
            ManifestVersion::parse("2026.6.10.1").unwrap()
                > ManifestVersion::parse("2026.6.9.9").unwrap()
        );
        assert!(ManifestVersion::parse("1.2.0").unwrap() == ManifestVersion::parse("1.2").unwrap());
        assert!(ManifestVersion::parse("1.2.1").unwrap() > ManifestVersion::parse("1.2").unwrap());
    }

    #[test]
    fn manifest_version_rejects_non_numeric_segments() {
        assert!(ManifestVersion::parse("").is_err());
        assert!(ManifestVersion::parse("2026.06.alpha").is_err());
        assert!(ManifestVersion::parse("2026..06").is_err());
        assert!(ManifestVersion::parse("2026.999999999999999999999999999999").is_err());
    }

    #[test]
    fn process_agent_manifest_commits_newer_manifest_atomically() {
        with_state_dir("commit-newer", || {
            let content = remote_manifest("9999.01.01.1", "ready");
            let commit = process_agent_manifest(Agent::Codex, &content, 1)
                .unwrap()
                .unwrap();

            assert_eq!(commit.agent, Agent::Codex);
            assert_eq!(
                commit.version,
                ManifestVersion::parse("9999.01.01.1").unwrap()
            );
            assert_eq!(
                fs::read_to_string(remote_manifest_path(Agent::Codex)).unwrap(),
                content
            );
        });
    }

    #[test]
    fn auto_update_reloads_manifest_cache_after_remote_commit() {
        with_state_dir("auto-update-reloads-cache", || {
            let web_dir = std::env::temp_dir()
                .join(format!("h0x-manifest-update-web-{}", std::process::id()));
            let _ = fs::remove_dir_all(&web_dir);
            fs::create_dir_all(&web_dir).unwrap();
            fs::write(
                web_dir.join("index.toml"),
                r#"
schema_version = 1

[[agents]]
id = "codex"
path = "codex.toml"
"#,
            )
            .unwrap();
            fs::write(
                web_dir.join("codex.toml"),
                remote_manifest("9999.01.01.1", "auto-update-ready"),
            )
            .unwrap();
            let url = file_url(&web_dir);
            put_sig(&web_dir, "index.toml", SIG_IDX_A);
            put_sig(&web_dir, "codex.toml", SIG_M_READY);

            let (tx, mut rx) = tokio::sync::mpsc::channel(1);
            auto_update_from(tx, &url, TEST_KEY);

            let event = rx.try_recv().expect("manifest update event");
            let crate::events::AppEvent::AgentDetectionManifestsUpdated { updated, .. } = event
            else {
                panic!("unexpected event");
            };
            assert_eq!(updated.len(), 1);
            assert_eq!(updated[0].agent, Agent::Codex);

            let explain = crate::detect::manifest::explain(Agent::Codex, "auto-update-ready");
            assert_eq!(explain.state, crate::detect::AgentState::Idle);
            assert!(matches!(
                explain.source,
                Some(crate::detect::manifest::ManifestSource::Remote { .. })
            ));
            assert_eq!(
                explain.matched_rule.as_ref().map(|rule| rule.id.as_str()),
                Some("idle")
            );

            let _ = fs::remove_dir_all(&web_dir);
        });
    }

    #[test]
    fn auto_update_reloads_manifest_cache_when_remote_is_already_current() {
        with_state_dir("auto-update-reloads-current-cache", || {
            let initial = remote_manifest("9999.01.01.1", "initial-ready");
            process_agent_manifest(Agent::Codex, &initial, 1).unwrap();
            crate::detect::manifest::reload_manifests();

            let web_dir = std::env::temp_dir().join(format!(
                "h0x-manifest-update-current-web-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&web_dir);
            fs::create_dir_all(&web_dir).unwrap();
            fs::write(
                web_dir.join("index.toml"),
                r#"
schema_version = 1

[[agents]]
id = "codex"
path = "codex.toml"
"#,
            )
            .unwrap();
            let current = remote_manifest("9999.01.01.2", "current-ready");
            fs::write(web_dir.join("codex.toml"), &current).unwrap();
            fs::write(remote_manifest_path(Agent::Codex), current).unwrap();
            let url = file_url(&web_dir);
            put_sig(&web_dir, "index.toml", SIG_IDX_A);
            put_sig(&web_dir, "codex.toml", SIG_M_CURRENT);

            let (tx, mut rx) = tokio::sync::mpsc::channel(1);
            auto_update_from(tx, &url, TEST_KEY);

            let event = rx.try_recv().expect("manifest update event");
            let crate::events::AppEvent::AgentDetectionManifestsUpdated { updated, .. } = event
            else {
                panic!("unexpected event");
            };
            assert!(updated.is_empty());

            let explain = crate::detect::manifest::explain(Agent::Codex, "current-ready");
            assert_eq!(explain.state, crate::detect::AgentState::Idle);
            assert_eq!(explain.manifest_version.as_deref(), Some("9999.01.01.2"));
            assert_eq!(
                explain.matched_rule.as_ref().map(|rule| rule.id.as_str()),
                Some("idle")
            );

            let _ = fs::remove_dir_all(&web_dir);
        });
    }

    #[test]
    fn auto_update_does_not_reload_agents_whose_check_failed() {
        with_state_dir("auto-update-skips-failed-agent", || {
            let initial_codex = remote_manifest("9999.01.01.1", "codex-initial-ready");
            process_agent_manifest(Agent::Codex, &initial_codex, 1).unwrap();
            let initial_cursor =
                remote_manifest_for("cursor", "9999.01.01.1", "cursor-initial-ready");
            process_agent_manifest(Agent::Cursor, &initial_cursor, 1).unwrap();
            crate::detect::manifest::reload_manifests();

            let web_dir = std::env::temp_dir().join(format!(
                "h0x-manifest-update-partial-web-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&web_dir);
            fs::create_dir_all(&web_dir).unwrap();
            fs::write(
                web_dir.join("index.toml"),
                r#"
schema_version = 1

[[agents]]
id = "codex"
path = "codex.toml"

[[agents]]
id = "cursor"
path = "missing-cursor.toml"
"#,
            )
            .unwrap();
            let current_codex = remote_manifest("9999.01.01.2", "codex-current-ready");
            fs::write(web_dir.join("codex.toml"), &current_codex).unwrap();
            fs::write(remote_manifest_path(Agent::Codex), current_codex).unwrap();
            fs::write(
                remote_manifest_path(Agent::Cursor),
                remote_manifest_for("cursor", "9999.01.01.2", "cursor-current-ready"),
            )
            .unwrap();
            let url = file_url(&web_dir);
            put_sig(&web_dir, "index.toml", SIG_IDX_B);
            put_sig(&web_dir, "codex.toml", SIG_M_CODEX_CURRENT);

            let (tx, mut rx) = tokio::sync::mpsc::channel(1);
            auto_update_from(tx, &url, TEST_KEY);

            let event = rx.try_recv().expect("manifest update event");
            let crate::events::AppEvent::AgentDetectionManifestsUpdated {
                updated, activated, ..
            } = event
            else {
                panic!("unexpected event");
            };
            assert!(updated.is_empty());
            assert_eq!(activated, vec![Agent::Codex]);

            let codex = crate::detect::manifest::explain(Agent::Codex, "codex-current-ready");
            assert_eq!(codex.manifest_version.as_deref(), Some("9999.01.01.2"));
            assert_eq!(
                codex.matched_rule.as_ref().map(|rule| rule.id.as_str()),
                Some("idle")
            );
            let cursor = crate::detect::manifest::explain(Agent::Cursor, "cursor-initial-ready");
            assert_eq!(cursor.manifest_version.as_deref(), Some("9999.01.01.1"));
            assert_eq!(
                cursor.matched_rule.as_ref().map(|rule| rule.id.as_str()),
                Some("idle")
            );

            let _ = fs::remove_dir_all(&web_dir);
        });
    }

    #[test]
    fn cache_reload_ignores_retained_status_for_unchecked_agent() {
        with_state_dir("cache-reload-ignores-retained-status", || {
            let initial = remote_manifest("9999.01.01.1", "initial-ready");
            process_agent_manifest(Agent::Codex, &initial, 1).unwrap();
            crate::detect::manifest::reload_manifests();
            fs::write(
                remote_manifest_path(Agent::Codex),
                remote_manifest("9999.01.01.2", "current-ready"),
            )
            .unwrap();

            let mut status = ManifestUpdateStatus::default();
            status.agents.insert(
                "codex".to_string(),
                AgentRemoteStatus {
                    cached_version: Some("9999.01.01.1".to_string()),
                    attempted_version: None,
                    last_checked_unix: Some(1),
                    last_result: "current".to_string(),
                    last_error: None,
                },
            );
            let output = ManifestUpdateOutput {
                checked: vec![Agent::Cursor],
                updated: Vec::new(),
                status,
            };

            assert!(agents_needing_cache_reload(&output).is_empty());
        });
    }

    #[test]
    fn process_agent_manifest_rejects_downgrade_and_keeps_cached_manifest() {
        with_state_dir("reject-downgrade", || {
            let current = remote_manifest("9999.01.01.1", "current");
            process_agent_manifest(Agent::Codex, &current, 1).unwrap();

            let older = remote_manifest("9999.01.01.0", "older");
            assert!(process_agent_manifest(Agent::Codex, &older, 2).is_err());
            assert_eq!(
                fs::read_to_string(remote_manifest_path(Agent::Codex)).unwrap(),
                current
            );
        });
    }

    #[test]
    fn process_agent_manifest_rejects_equal_version_content_change() {
        with_state_dir("reject-equal-change", || {
            let current = remote_manifest("9999.01.01.1", "current");
            process_agent_manifest(Agent::Codex, &current, 1).unwrap();

            let changed = remote_manifest("9999.01.01.1", "changed");
            assert!(process_agent_manifest(Agent::Codex, &changed, 2).is_err());
            assert_eq!(
                fs::read_to_string(remote_manifest_path(Agent::Codex)).unwrap(),
                current
            );
        });
    }

    #[test]
    fn process_agent_manifest_skips_same_version_same_content() {
        with_state_dir("skip-same", || {
            let current = remote_manifest("9999.01.01.1", "current");
            process_agent_manifest(Agent::Codex, &current, 1).unwrap();

            let result = process_agent_manifest(Agent::Codex, &current, 2).unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn catalog_parses_known_agents_and_rejects_duplicates() {
        let catalog = parse_catalog(
            r#"
schema_version = 1

[[agents]]
id = "codex"
path = "codex.toml"
"#,
        )
        .unwrap();
        assert_eq!(catalog[0].agent, Agent::Codex);
        assert_eq!(catalog[0].path, "codex.toml");

        assert!(parse_catalog(
            r#"
schema_version = 1

[[agents]]
id = "codex"
path = "codex.toml"

[[agents]]
id = "codex"
path = "codex-2.toml"
"#
        )
        .is_err());
    }

    #[test]
    fn catalog_rejects_unsafe_paths() {
        assert!(parse_catalog(
            r#"
schema_version = 1

[[agents]]
id = "codex"
path = "../codex.toml"
"#
        )
        .is_err());
    }

    #[test]
    fn catalog_rejects_absolute_foreign_host_and_empty_manifest_paths() {
        for path in [
            "",
            "   ",
            "/etc/codex.toml",
            "//evil.example/codex.toml",
            "https://evil.example/codex.toml",
            "file:///etc/codex.toml",
            "sub/../../codex.toml",
            "sub/..",
        ] {
            let catalog =
                format!("schema_version = 1\n\n[[agents]]\nid = \"codex\"\npath = \"{path}\"\n");
            assert!(
                parse_catalog(&catalog).is_err(),
                "catalog accepted {path:?}"
            );
            if !path.trim().is_empty() {
                assert!(
                    join_url("https://example.invalid/download", path).is_err(),
                    "join_url accepted {path:?}"
                );
            }
        }
    }

    #[test]
    fn signatures_with_crlf_line_endings_are_accepted() {
        with_state_dir("sig-crlf", || {
            let manifest = remote_manifest("9999.01.01.2", "current-ready");
            let index_sig = SIG_IDX_A.replace('\n', "\r\n");
            let codex_sig = SIG_M_CURRENT.replace('\n', "\r\n");
            let output = serve_and_update(
                "crlf",
                (INDEX_A, Some(&index_sig)),
                (&manifest, Some(&codex_sig)),
                TEST_KEY,
            )
            .unwrap();
            assert_eq!(output.updated.len(), 1);
            assert_eq!(cached_codex_version().as_deref(), Some("9999.01.01.2"));
        });
    }

    #[test]
    fn signed_empty_catalog_changes_nothing() {
        let manifest = remote_manifest("9999.01.01.2", "current-ready");
        with_state_dir("sig-empty-file", || {
            let err = serve_and_update(
                "empty-file",
                ("", Some(SIG_IDX_EMPTY)),
                (&manifest, Some(SIG_M_CURRENT)),
                TEST_KEY,
            )
            .expect_err("an empty catalog file has no schema_version");
            assert!(err.contains("catalog"), "{err}");
            assert_eq!(cached_codex_version().as_deref(), Some("9999.01.01.1"));
        });
        with_state_dir("sig-no-agents", || {
            let output = serve_and_update(
                "no-agents",
                ("\nschema_version = 1\n", Some(SIG_IDX_NO_AGENTS)),
                (&manifest, Some(SIG_M_CURRENT)),
                TEST_KEY,
            )
            .unwrap();
            assert!(output.checked.is_empty());
            assert!(output.updated.is_empty());
            assert_eq!(cached_codex_version().as_deref(), Some("9999.01.01.1"));
        });
    }

    #[test]
    fn catalog_is_read_up_to_the_fetch_limit_and_refused_beyond_it() {
        let manifest = remote_manifest("9999.01.01.2", "current-ready");
        let padded_index = |len: usize| {
            let mut index = format!("{INDEX_A}#");
            while index.len() < len - 1 {
                index.push('x');
            }
            index.push('\n');
            index
        };
        with_state_dir("sig-size-at-limit", || {
            let index = padded_index(MAX_FETCH_BYTES);
            assert_eq!(index.len(), MAX_FETCH_BYTES);
            let output = serve_and_update(
                "size-at-limit",
                (&index, Some(SIG_IDX_AT_LIMIT)),
                (&manifest, Some(SIG_M_CURRENT)),
                TEST_KEY,
            )
            .unwrap();
            assert_eq!(output.updated.len(), 1);
            assert_eq!(cached_codex_version().as_deref(), Some("9999.01.01.2"));
        });
        with_state_dir("sig-size-over-limit", || {
            let index = padded_index(MAX_FETCH_BYTES + 1);
            assert_eq!(index.len(), MAX_FETCH_BYTES + 1);
            let result = serve_and_update(
                "size-over-limit",
                (&index, Some(SIG_IDX_OVER_LIMIT)),
                (&manifest, Some(SIG_M_CURRENT)),
                TEST_KEY,
            );
            assert!(result.is_err(), "{result:?}");
            assert_eq!(cached_codex_version().as_deref(), Some("9999.01.01.1"));
        });
    }

    #[test]
    fn signed_but_invalid_manifest_fails_only_that_agent() {
        let bad_regex = format!(
            "{}regex = [\"(\"]\n",
            remote_manifest("9999.01.01.2", "bad-regex")
        );
        let cursor = remote_manifest_for("cursor", "9999.01.01.2", "cursor-current-ready");
        let cases: [(&str, &str, &str); 3] = [
            (
                "bad-toml",
                "id = \"codex\"\nversion =\n",
                SIG_M_CODEX_BAD_TOML,
            ),
            ("bad-regex", &bad_regex, SIG_M_CODEX_BAD_REGEX),
            // The cursor manifest, signed as `codex.toml`: a valid signature
            // does not let one agent's manifest stand in for another's.
            ("other-agent", &cursor, SIG_M_CURSOR_AS_CODEX),
        ];
        for (name, codex, codex_sig) in cases {
            with_state_dir(&format!("sig-invalid-{name}"), || {
                for agent in [Agent::Codex, Agent::Cursor] {
                    let initial =
                        remote_manifest_for(agent_label(agent), "9999.01.01.1", "initial-ready");
                    process_agent_manifest(agent, &initial, 1).unwrap();
                }
                let dir = web_dir(&format!("invalid-{name}"));
                fs::write(dir.join("index.toml"), INDEX_TWO).unwrap();
                put_sig(&dir, "index.toml", SIG_IDX_TWO);
                fs::write(dir.join("codex.toml"), codex).unwrap();
                put_sig(&dir, "codex.toml", codex_sig);
                fs::write(dir.join("cursor.toml"), &cursor).unwrap();
                put_sig(&dir, "cursor.toml", SIG_M_CURSOR);
                let result = check_and_update_from_url(&file_url(&dir), TEST_KEY);
                let _ = fs::remove_dir_all(&dir);

                let output = result.unwrap();
                let updated = output.updated.iter().map(|c| c.agent).collect::<Vec<_>>();
                assert_eq!(updated, vec![Agent::Cursor], "{name}");
                assert_eq!(
                    cached_codex_version().as_deref(),
                    Some("9999.01.01.1"),
                    "{name}"
                );
                assert_eq!(
                    cached_remote_version(Agent::Cursor)
                        .map(|v| v.to_string())
                        .as_deref(),
                    Some("9999.01.01.2"),
                    "{name}"
                );
                let codex_status = output.status.agents.get("codex").unwrap();
                assert_eq!(codex_status.last_result, "failed", "{name}");
                assert!(
                    codex_status
                        .last_error
                        .as_deref()
                        .is_some_and(|e| !e.contains("not signed")),
                    "{name}: the signature was valid: {codex_status:?}"
                );
                assert_eq!(
                    output.status.agents.get("cursor").unwrap().last_result,
                    "updated",
                    "{name}"
                );
            });
        }
    }

    #[test]
    fn manifest_path_with_a_query_string_is_not_applied() {
        with_state_dir("sig-query-path", || {
            let initial = remote_manifest("9999.01.01.1", "initial-ready");
            process_agent_manifest(Agent::Codex, &initial, 1).unwrap();
            // A correctly signed `codex.toml` sits next to the catalog, but the
            // catalog names it `codex.toml?v=1`, which is not the signed name.
            let dir = web_dir("query-path");
            let manifest = remote_manifest("9999.01.01.2", "current-ready");
            fs::write(dir.join("codex.toml"), &manifest).unwrap();
            put_sig(&dir, "codex.toml", SIG_M_CURRENT);
            fs::write(dir.join("index.toml"), INDEX_QUERY).unwrap();
            put_sig(&dir, "index.toml", SIG_IDX_QUERY);
            let result = check_and_update_from_url(&file_url(&dir), TEST_KEY);
            let _ = fs::remove_dir_all(&dir);

            let output = result.unwrap();
            assert!(output.updated.is_empty(), "applied {output:?}");
            let status = output.status.agents.get("codex").unwrap();
            assert_eq!(status.last_result, "failed");
            assert_eq!(cached_codex_version().as_deref(), Some("9999.01.01.1"));
        });
    }

    #[test]
    fn manifest_paths_cannot_leave_the_release_directory() {
        let manifest = remote_manifest("9999.01.01.2", "current-ready");
        let cases: [(&str, &str, &str); 2] = [
            // A Windows separator that `..` checks on `/` do not see.
            ("backslash", INDEX_BACKSLASH, SIG_IDX_BACKSLASH),
            ("query", INDEX_QUERY, SIG_IDX_QUERY),
        ];
        for (name, index, index_sig) in cases {
            with_state_dir(&format!("sig-escape-{name}"), || {
                let initial = remote_manifest("9999.01.01.1", "initial-ready");
                process_agent_manifest(Agent::Codex, &initial, 1).unwrap();
                let dir = web_dir(&format!("escape-{name}"));
                let release = dir.join("release");
                let outside = dir.join("outside");
                // A correctly signed codex manifest both inside and outside
                // the release directory.
                for place in [&release, &outside] {
                    fs::create_dir_all(place).unwrap();
                    fs::write(place.join("codex.toml"), &manifest).unwrap();
                    put_sig(place, "codex.toml", SIG_M_CURRENT);
                }
                fs::write(release.join("index.toml"), index).unwrap();
                put_sig(&release, "index.toml", index_sig);
                let result = check_and_update_from_url(&file_url(&release), TEST_KEY);
                let _ = fs::remove_dir_all(&dir);

                if let Ok(output) = &result {
                    assert!(output.updated.is_empty(), "{name}: applied {output:?}");
                    let status = output.status.agents.get("codex").unwrap();
                    assert_eq!(status.last_result, "failed", "{name}");
                }
                assert_eq!(
                    cached_codex_version().as_deref(),
                    Some("9999.01.01.1"),
                    "{name}"
                );
            });
        }
    }

    const INDEX_TWO: &str = r#"
schema_version = 1

[[agents]]
id = "codex"
path = "codex.toml"

[[agents]]
id = "cursor"
path = "cursor.toml"
"#;

    const INDEX_BACKSLASH: &str = r#"
schema_version = 1

[[agents]]
id = "codex"
path = '..\outside/codex.toml'
"#;

    const INDEX_QUERY: &str = r#"
schema_version = 1

[[agents]]
id = "codex"
path = "codex.toml?v=1"
"#;

    const SIG_IDX_TWO: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YM/YP2ux75pDoShxvEXzGf9kBxqn0Z+nnwsMOO3nViYdrpCdJLotu8bw515107bStJvB+J/r0vkXds3ehW32DAQ=
trusted comment: timestamp:1\tfile:index.toml
xWxPLWHLcuRQwkNLXAVVY/TymqYKV/RNa24UgPnY2OoXR/WUhP7FYTOQo0i5sqF4SL8nTNsyHBN4BxLWnGx5Dg==
";
    const SIG_IDX_BACKSLASH: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YM2nJU+w1rAD0TnkuXk6m/Lvdlm5bOtADioeJf4G+DJvIpzOM11NiVs+G3/LdKAbW3c1G7t+yQNpFzM70jaE6gU=
trusted comment: timestamp:1\tfile:index.toml
s7Bx+MosiplRvZj0lw3cXv9lpJxId93+rCPaURLMfcxXBByTO6aPpc0h0kNtvCEfTMIJZ8JxR1hRHVUHm3LnCA==
";
    const SIG_IDX_QUERY: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YNtSxQw0se6o9xHYMtZQF99Fx3D943sSW7Wk7PAmcDpjYH6NH8D8ZsvZoyy6PUT6zerlU8pEXidaWy8GuKZZkg8=
trusted comment: timestamp:1\tfile:index.toml
gnfZLXoWPWSxYxyK68vXZ6qtD/TBf/yMB7AwJAFTPXdcou2NfBVHKzvp82hNLQYawdlEgfb8VbulqAzwvzvTBQ==
";
    const SIG_IDX_EMPTY: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YFMBAJ8ruuPc6a3mLf5XTC/UfJ8ksyV7CxcA+eG/b3mExha35vS22Y6J/Q+/Rj/phqlnlboyk47QUsxaBcXqUws=
trusted comment: timestamp:1\tfile:index.toml
ZtuQCTzwjQ+3Dvo6wbrAJwcqR/46xSUAXT2CyvNt2OtOG817eu86SHJKPXAuxHDBonSjA3aybjjzxEcwZOoJDg==
";
    const SIG_IDX_NO_AGENTS: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YOcrrzBAU7ZPAQsroxtXI01LVL0Nd8Jl73Mj6xKuvHDNuWOw8g9V9ijNrXpnMV1/BjFTpC0NV5K1v00X4tDWzwo=
trusted comment: timestamp:1\tfile:index.toml
4VyVaY563a5n29EcCaXhH4MtFcP80K0+mD1ioKKFy2MOzMoeN2WZ6F7ucAcbXtWlPCbVDgfRHtM8Tq/UsHiLAg==
";
    const SIG_IDX_AT_LIMIT: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YDR/GhNAaNfnaq6XlXybjKgqwq5IgzNEGZYNEY5u0MJj/qQp9YSbXLmTX/baX6VEElH8rTUZLxZgZFF64reHYAo=
trusted comment: timestamp:1\tfile:index.toml
YpzFKNKNJ/3vj+zKjXHYYvFpVAxeSm01zjflzlb0fZj0i27tRtA5pnPgVYrI4peyRo6pnQhdWkt2XNMYxTBcDw==
";
    const SIG_IDX_OVER_LIMIT: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YAJMvIYneHE7YLjOr/CNZ3K/y1LU4Bn8lypJVUUeIcnLM/xU/3q8jt7PcUk0xHOV2h5rbrndhvtnLAAdVWnMVg4=
trusted comment: timestamp:1\tfile:index.toml
p6cpphCzVa1w8LwUQpWscLdb1uihjfnNPlzX904OBLWKY80XJSnoGvY8kklTBUnwuR/7YXXP+rXCGiNAvJE3Bg==
";
    const SIG_M_CODEX_BAD_TOML: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YAwrKJ6UEy7Tkjh28TFdGxmUgLQFr43nMRtgo28RlisgD+lwNN8KJc+qJ8ZRm6KqgXHSGYkxu8wxqzB00wXhrgE=
trusted comment: timestamp:1\tfile:codex.toml
7x5wG2ESEjQmhgzv9k9f0z2qrVyJOO7czxQEJCfgCbp8KVHic4nKJgRvNjmz9GLrEuRhOkathYtGrLL1+WXnCA==
";
    const SIG_M_CODEX_BAD_REGEX: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YFKSxg9VEQNr9qsGYQmHBYUmnelUk6WYmXai+Vr+WR1UW72Bsbqz5cMmAHao6sbnMAgd8x3LOm9MsK0fMiYzFQI=
trusted comment: timestamp:1\tfile:codex.toml
K9QRN3VWNmS/CfLacCX1Y171fJbBOvaA7HeRipY2HOH6hDHl2Leob9KT+mt1b+ZwO862rC+43EDRte/QLV2UAw==
";
    const SIG_M_CURSOR_AS_CODEX: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YJy7HD+8x2FmFUJqfU0EHuFHGwUrevunXaZ9fxXpdY37IEAqDm7mbc4CZyhzpFXBUFp3BOsrxBgK7TB8EFXeZQs=
trusted comment: timestamp:1\tfile:codex.toml
nGlbYu+aWZQYShfJh6Y9sNkNQaSGtd0rP9Ya7DzUdpw6ycLzAYeGmQ+HBTU4ysA35Y2e+zVEfOHDXWGnjrymAA==
";
    const SIG_M_CURSOR: &str = "untrusted comment: signature from rsign secret key
RURoiCZgYc65YIwcVrJrrLv1wDDdXgPB5mQrOxfiTo36iS5KajYWpKx5eLlo6DgSUmnl3SdWKzvnxXwgvQxUrUeFpDubUoUiPAA=
trusted comment: timestamp:1\tfile:cursor.toml
MWBBe11zeMnCa+rjy2QTKodaEEAxfyHlvHZUFpWGCWEz2jYIxfgfOIc1g89dEOOUTy/cK4Q9MxntRRjfZFc2BA==
";
}
