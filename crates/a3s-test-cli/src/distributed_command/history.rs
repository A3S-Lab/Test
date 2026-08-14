use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use a3s_test_worker::{DistributedHistoryRun, DistributedRunAnalysis};
use anyhow::{Context, Result};
use fs2::FileExt as _;

const HISTORY_PROTOCOL: &str = "a3s.test.distributed-history/1";
const MAX_HISTORY_RUN_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ANALYSIS_REPORT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_HISTORY_DIRECTORY_ENTRIES: usize = 1_000;
static NEXT_STAGING_FILE: AtomicU64 = AtomicU64::new(1);

pub(super) struct HistoryStore {
    root: PathBuf,
    runs: PathBuf,
    reports: PathBuf,
    _lock: std::fs::File,
}

#[derive(serde::Serialize)]
#[serde(deny_unknown_fields)]
struct StoredHistoryRun<'a> {
    protocol: &'static str,
    run: &'a DistributedHistoryRun,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LoadedHistoryRun {
    protocol: String,
    run: DistributedHistoryRun,
}

impl HistoryStore {
    pub(super) async fn open(root: PathBuf) -> Result<Self> {
        let root_for_create = root.clone();
        tokio::task::spawn_blocking(move || {
            ensure_private_directory(&root_for_create)?;
            set_private_directory_permissions(&root_for_create)
        })
        .await
        .context("distributed history directory task failed")??;
        let root = tokio::fs::canonicalize(&root)
            .await
            .context("failed to resolve distributed history root")?;
        let runs = root.join("runs");
        let reports = root.join("reports");
        for directory in [&runs, &reports] {
            let directory = directory.clone();
            tokio::task::spawn_blocking(move || {
                ensure_private_directory(&directory)?;
                set_private_directory_permissions(&directory)
            })
            .await
            .context("distributed history subdirectory task failed")??;
        }
        let lock_path = root.join("history.lock");
        let lock = tokio::task::spawn_blocking(move || -> Result<std::fs::File> {
            if let Ok(metadata) = std::fs::symlink_metadata(&lock_path) {
                if !metadata.is_file() || is_link_like(&metadata) {
                    anyhow::bail!("distributed history lock must be a regular non-link file");
                }
            }
            let file = OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(&lock_path)
                .with_context(|| format!("failed to open history lock {}", lock_path.display()))?;
            let path_metadata = std::fs::symlink_metadata(&lock_path)
                .context("failed to inspect history lock path")?;
            let metadata = file.metadata().context("failed to inspect history lock")?;
            if !path_metadata.is_file() || is_link_like(&path_metadata) || !metadata.is_file() {
                anyhow::bail!("distributed history lock must be a regular non-link file");
            }
            set_private_file_permissions(&file)?;
            file.try_lock_exclusive().map_err(|error| {
                anyhow::anyhow!("another distributed run owns this history root: {error}")
            })?;
            Ok(file)
        })
        .await
        .context("distributed history lock task failed")??;
        Ok(Self {
            root,
            runs,
            reports,
            _lock: lock,
        })
    }

    pub(super) async fn load(
        &self,
        now_ms: u64,
        max_runs: usize,
        max_age_ms: u64,
    ) -> Result<Vec<DistributedHistoryRun>> {
        let mut runs = self.read_all().await?;
        runs.retain(|run| {
            run.finished_at_ms < now_ms && now_ms.saturating_sub(run.finished_at_ms) <= max_age_ms
        });
        runs.sort_by_key(|run| (std::cmp::Reverse(run.finished_at_ms), run.run_id.clone()));
        runs.truncate(max_runs);
        Ok(runs)
    }

    pub(super) async fn persist(
        &self,
        analysis: &DistributedRunAnalysis,
        max_runs: usize,
        max_age_ms: u64,
    ) -> Result<()> {
        let history = serde_json::to_vec_pretty(&StoredHistoryRun {
            protocol: HISTORY_PROTOCOL,
            run: &analysis.history_record,
        })
        .context("failed to encode distributed history record")?;
        let report = serde_json::to_vec_pretty(analysis)
            .context("failed to encode distributed analysis report")?;
        write_new_or_identical(
            &self.runs.join(format!("{}.json", analysis.run_id)),
            &history,
            MAX_HISTORY_RUN_BYTES,
        )
        .await?;
        write_new_or_identical(
            &self.reports.join(format!("{}.json", analysis.run_id)),
            &report,
            MAX_ANALYSIS_REPORT_BYTES,
        )
        .await?;
        sync_directory(&self.runs).await?;
        sync_directory(&self.reports).await?;
        self.enforce_retention(analysis.finished_at_ms, max_runs, max_age_ms)
            .await
    }

    pub(super) fn report_path(&self, run_id: &str) -> PathBuf {
        self.reports.join(format!("{run_id}.json"))
    }

    async fn read_all(&self) -> Result<Vec<DistributedHistoryRun>> {
        let mut entries = tokio::fs::read_dir(&self.runs)
            .await
            .context("failed to read distributed history directory")?;
        let mut runs = Vec::new();
        let mut count = 0_usize;
        while let Some(entry) = entries
            .next_entry()
            .await
            .context("failed to enumerate distributed history directory")?
        {
            count += 1;
            if count > MAX_HISTORY_DIRECTORY_ENTRIES {
                anyhow::bail!("distributed history directory exceeds its entry bound");
            }
            let path = entry.path();
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("history filename is not portable UTF-8"))?;
            let Some(run_id) = name.strip_suffix(".json") else {
                anyhow::bail!("distributed history directory contains an unknown entry");
            };
            validate_run_id(run_id)?;
            let metadata = tokio::fs::symlink_metadata(&path)
                .await
                .with_context(|| format!("failed to inspect history file {}", path.display()))?;
            if !metadata.is_file()
                || is_link_like(&metadata)
                || metadata.len() == 0
                || metadata.len() > MAX_HISTORY_RUN_BYTES
            {
                anyhow::bail!("distributed history contains an unsafe or oversized file");
            }
            let bytes = tokio::fs::read(&path)
                .await
                .with_context(|| format!("failed to read history file {}", path.display()))?;
            if bytes.is_empty() || bytes.len() as u64 > MAX_HISTORY_RUN_BYTES {
                anyhow::bail!("distributed history changed size while being read");
            }
            let stored: LoadedHistoryRun = serde_json::from_slice(&bytes)
                .with_context(|| format!("invalid history file {}", path.display()))?;
            if stored.protocol != HISTORY_PROTOCOL || stored.run.run_id != run_id {
                anyhow::bail!("distributed history file identity or protocol mismatch");
            }
            runs.push(stored.run);
        }
        Ok(runs)
    }

    async fn enforce_retention(&self, now_ms: u64, max_runs: usize, max_age_ms: u64) -> Result<()> {
        let mut runs = self.read_all().await?;
        runs.sort_by_key(|run| (std::cmp::Reverse(run.finished_at_ms), run.run_id.clone()));
        for (index, run) in runs.into_iter().enumerate() {
            let expired = run.finished_at_ms > now_ms
                || now_ms.saturating_sub(run.finished_at_ms) > max_age_ms;
            if index < max_runs && !expired {
                continue;
            }
            remove_regular_file(&self.runs.join(format!("{}.json", run.run_id))).await?;
            let report = self.reports.join(format!("{}.json", run.run_id));
            if tokio::fs::try_exists(&report)
                .await
                .context("failed to inspect retained distributed report")?
            {
                remove_regular_file(&report).await?;
            }
        }
        sync_directory(&self.root).await
    }
}

async fn write_new_or_identical(path: &Path, bytes: &[u8], max_bytes: u64) -> Result<()> {
    if bytes.is_empty() || bytes.len() as u64 > max_bytes {
        anyhow::bail!("distributed history output is empty or oversized");
    }
    if tokio::fs::try_exists(path)
        .await
        .with_context(|| format!("failed to inspect history output {}", path.display()))?
    {
        let metadata = tokio::fs::symlink_metadata(path)
            .await
            .with_context(|| format!("failed to inspect history output {}", path.display()))?;
        if !metadata.is_file()
            || is_link_like(&metadata)
            || metadata.len() == 0
            || metadata.len() > max_bytes
        {
            anyhow::bail!("existing distributed history output is unsafe");
        }
        let existing = tokio::fs::read(path)
            .await
            .with_context(|| format!("failed to read history output {}", path.display()))?;
        if existing.is_empty() || existing.len() as u64 > max_bytes {
            anyhow::bail!("existing distributed history output changed size while being read");
        }
        if existing == bytes {
            return Ok(());
        }
        anyhow::bail!("distributed history output already exists with different bytes");
    }
    let parent = path
        .parent()
        .context("distributed history output has no parent directory")?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("distributed history filename is not portable UTF-8")?;
    let staging = parent.join(format!(
        ".{file_name}.{}-{}.tmp",
        std::process::id(),
        NEXT_STAGING_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let staging_for_write = staging.clone();
    let bytes = bytes.to_vec();
    let write_result = tokio::task::spawn_blocking(move || -> Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&staging_for_write)
            .with_context(|| {
                format!(
                    "failed to create history staging file {}",
                    staging_for_write.display()
                )
            })?;
        set_private_file_permissions(&file)?;
        file.write_all(&bytes)
            .context("failed to write history staging file")?;
        file.sync_all()
            .context("failed to sync history staging file")
    })
    .await
    .context("distributed history write task failed")?;
    if let Err(error) = write_result {
        let _ = tokio::fs::remove_file(&staging).await;
        return Err(error);
    }
    if let Err(error) = tokio::fs::rename(&staging, path).await {
        let _ = tokio::fs::remove_file(&staging).await;
        return Err(error)
            .with_context(|| format!("failed to publish history output {}", path.display()));
    }
    Ok(())
}

async fn remove_regular_file(path: &Path) -> Result<()> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .with_context(|| format!("failed to inspect retained file {}", path.display()))?;
    if !metadata.is_file() || is_link_like(&metadata) {
        anyhow::bail!("refusing to remove an unsafe distributed history entry");
    }
    tokio::fs::remove_file(path)
        .await
        .with_context(|| format!("failed to prune retained file {}", path.display()))
}

fn ensure_private_directory(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        anyhow::bail!("distributed history directory must not be empty");
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            Component::Normal(component) => {
                current.push(component);
                match std::fs::symlink_metadata(&current) {
                    Ok(metadata) => {
                        if !metadata.is_dir() || is_link_like(&metadata) {
                            anyhow::bail!(
                                "distributed history path contains a non-directory or link"
                            );
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        std::fs::create_dir(&current).with_context(|| {
                            format!("failed to create history directory {}", current.display())
                        })?;
                        set_private_directory_permissions(&current)?;
                    }
                    Err(error) => return Err(error).context("failed to inspect history directory"),
                }
            }
            Component::CurDir => {}
            Component::ParentDir => {
                anyhow::bail!("distributed history path cannot contain parent traversal");
            }
        }
    }
    Ok(())
}

fn validate_run_id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        anyhow::bail!("distributed history run ID is not portable");
    }
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(file: &std::fs::File) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .context("failed to set private history file permissions")
}

#[cfg(not(unix))]
fn set_private_file_permissions(_file: &std::fs::File) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .context("failed to set private history directory permissions")
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(windows)]
fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(unix)]
async fn sync_directory(path: &Path) -> Result<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::File::open(&path)
            .with_context(|| format!("failed to open history directory {}", path.display()))?
            .sync_all()
            .with_context(|| format!("failed to sync history directory {}", path.display()))
    })
    .await
    .context("distributed history directory sync task failed")?
}

#[cfg(not(unix))]
async fn sync_directory(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::HistoryStore;
    use a3s_test_worker::{
        DistributedHistoryRun, DistributedHistoryScenario, DistributedRunAnalysis,
        DistributedRunCounts, DistributedRunStatus, DistributedScenarioOutcome,
        DISTRIBUTED_RUN_PROTOCOL,
    };

    const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn analysis(run_id: &str, finished_at_ms: u64) -> DistributedRunAnalysis {
        let history_record = DistributedHistoryRun {
            run_id: run_id.to_string(),
            suite_digest: DIGEST.to_string(),
            finished_at_ms,
            scenarios: vec![DistributedHistoryScenario {
                id: "checkout".to_string(),
                outcome: DistributedScenarioOutcome::Passed,
                duration_ms: 100,
            }],
        };
        DistributedRunAnalysis {
            protocol: DISTRIBUTED_RUN_PROTOCOL.to_string(),
            plan_id: "plan".to_string(),
            plan_digest: DIGEST.to_string(),
            run_id: run_id.to_string(),
            suite: "suite".to_string(),
            suite_digest: DIGEST.to_string(),
            started_at_ms: finished_at_ms - 100,
            finished_at_ms,
            status: DistributedRunStatus::Passed,
            baseline_run_id: None,
            counts: DistributedRunCounts {
                passed: 1,
                ..DistributedRunCounts::default()
            },
            scenarios: Vec::new(),
            removed_scenarios: Vec::new(),
            shard_issues: Vec::new(),
            history_record,
        }
    }

    #[tokio::test]
    async fn persists_loads_and_prunes_bounded_history_with_reports() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().canonicalize().expect("canonical temp");
        let store = HistoryStore::open(root.join("history"))
            .await
            .expect("history store");
        for (id, time) in [("run-1", 1_000), ("run-2", 2_000), ("run-3", 3_000)] {
            store
                .persist(&analysis(id, time), 2, 10_000)
                .await
                .expect("persist history");
        }
        let loaded = store.load(4_000, 2, 10_000).await.expect("loaded history");
        assert_eq!(
            loaded
                .iter()
                .map(|run| run.run_id.as_str())
                .collect::<Vec<_>>(),
            vec!["run-3", "run-2"]
        );
        assert!(!store.report_path("run-1").exists());
        assert!(store.report_path("run-3").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_linked_history_entries() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().canonicalize().expect("canonical temp");
        let store = HistoryStore::open(root.join("history"))
            .await
            .expect("history store");
        let outside = root.join("outside.json");
        std::fs::write(&outside, b"{}").expect("outside");
        symlink(&outside, root.join("history/runs/run-linked.json")).expect("link");
        let error = store
            .load(4_000, 10, 10_000)
            .await
            .expect_err("linked history");
        assert!(error.to_string().contains("unsafe"), "{error:#}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn hardens_existing_history_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().canonicalize().expect("canonical temp");
        let history = root.join("history");
        std::fs::create_dir_all(history.join("runs")).expect("runs directory");
        std::fs::create_dir_all(history.join("reports")).expect("reports directory");
        for directory in [&history, &history.join("runs"), &history.join("reports")] {
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o755))
                .expect("permissive fixture directory");
        }

        let store = HistoryStore::open(history).await.expect("history store");
        store
            .persist(&analysis("run-private", 2_000), 10, 10_000)
            .await
            .expect("persist history");

        for directory in [&store.root, &store.runs, &store.reports] {
            let mode = std::fs::metadata(directory)
                .expect("history directory metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o700, "{}", directory.display());
        }
        for file in [
            store.root.join("history.lock"),
            store.runs.join("run-private.json"),
            store.reports.join("run-private.json"),
        ] {
            let mode = std::fs::metadata(&file)
                .expect("history file metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "{}", file.display());
        }
    }
}
