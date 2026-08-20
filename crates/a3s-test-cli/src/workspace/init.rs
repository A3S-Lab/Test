use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use serde_json::json;
use tokio::io::AsyncWriteExt;

use super::config;
use super::discovery;
use super::InitArgs;

pub(super) async fn execute(args: InitArgs) -> Result<ExitCode> {
    let discovered = discovery::discover(
        &args.root,
        args.script.as_deref(),
        args.url.as_deref(),
        args.testkit,
    )
    .await?;
    let config_path = init_config_path(&discovered.root, &args.config).await?;
    admit_destination(&config_path, args.force).await?;
    let config_parent = config_path
        .parent()
        .context("project profile path must have a parent directory")?;
    create_contained_directories(&discovered.root, config_parent).await?;
    let canonical_parent = tokio::fs::canonicalize(config_parent)
        .await
        .with_context(|| format!("failed to resolve {}", config_parent.display()))?;
    if !canonical_parent.starts_with(&discovered.root) {
        anyhow::bail!("project profile must stay inside the project root");
    }
    let root_reference = relative_root_reference(&discovered.root, &canonical_parent)?;
    let source = config::render(&discovered, &root_reference);
    atomic_write(&config_path, source.as_bytes(), args.force).await?;
    config::load(
        &discovered.root,
        relative_config(&discovered.root, &config_path)?,
    )
    .await?;

    let install = discovered.package_manager.install_command();
    let next = if discovered.testkit_required && !discovered.testkit_declared {
        vec![
            install.clone(),
            "a3s-test doctor".to_string(),
            "a3s-test dev".to_string(),
        ]
    } else {
        vec!["a3s-test doctor".to_string(), "a3s-test dev".to_string()]
    };
    let result = json!({
        "protocol": "a3s.test.project-init/1",
        "status": "initialized",
        "config_path": config_path,
        "project": {
            "id": discovered.id,
            "root": discovered.root,
            "framework": discovered.framework,
            "package_manager": discovered.package_manager,
            "script": discovered.script,
            "url": discovered.url.to_string(),
            "testkit_required": discovered.testkit_required,
            "testkit_declared": discovered.testkit_declared,
        },
        "next": next,
    });
    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "Initialized A3S Test project profile at {}",
            config_path.display()
        );
        if discovered.testkit_required && !discovered.testkit_declared {
            println!("Install Test Kit: {install}");
        }
        println!("Next: a3s-test doctor");
        println!("Then: a3s-test dev");
    }
    Ok(ExitCode::SUCCESS)
}

async fn init_config_path(root: &Path, configured: &Path) -> Result<PathBuf> {
    let relative = if configured.is_absolute() {
        configured
            .strip_prefix(root)
            .context("--config must stay inside --root")?
    } else {
        configured
    };
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        anyhow::bail!("--config must be a contained path inside --root");
    }
    let requested = root.join(relative);
    if requested.file_name().is_none() {
        anyhow::bail!("--config must name a project profile file");
    }
    Ok(requested)
}

async fn create_contained_directories(root: &Path, directory: &Path) -> Result<()> {
    let canonical_root = tokio::fs::canonicalize(root)
        .await
        .with_context(|| format!("failed to resolve project root {}", root.display()))?;
    let relative = directory
        .strip_prefix(root)
        .context("project profile directory is outside the project root")?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::CurDir => continue,
            Component::Normal(name) => current.push(name),
            _ => anyhow::bail!("project profile directory must stay inside the project root"),
        }
        loop {
            match tokio::fs::symlink_metadata(&current).await {
                Ok(metadata)
                    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() =>
                {
                    break;
                }
                Ok(_) => anyhow::bail!(
                    "project profile directory component {} must be a regular directory",
                    current.display()
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    match tokio::fs::create_dir(&current).await {
                        Ok(()) => break,
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                        Err(error) => {
                            return Err(error).with_context(|| {
                                format!("failed to create {}", current.display())
                            });
                        }
                    }
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to inspect {}", current.display()));
                }
            }
        }
    }
    let canonical = tokio::fs::canonicalize(directory)
        .await
        .with_context(|| format!("failed to resolve {}", directory.display()))?;
    if !canonical.starts_with(&canonical_root) {
        anyhow::bail!("project profile directory must stay inside the project root");
    }
    Ok(())
}

async fn admit_destination(path: &Path, force: bool) -> Result<()> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => {
            if !force {
                anyhow::bail!(
                    "project profile {} already exists; pass --force to replace it",
                    path.display()
                );
            }
            if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                anyhow::bail!("--force can replace only a regular non-link project profile");
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect project profile {}", path.display()));
        }
    }
    Ok(())
}

async fn atomic_write(path: &Path, bytes: &[u8], replace: bool) -> Result<()> {
    let parent = path
        .parent()
        .context("project profile path has no parent")?;
    let temporary = parent.join(format!(
        ".project.acl.{}.{}.tmp",
        std::process::id(),
        monotonic_suffix()
    ));
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .await
        .with_context(|| format!("failed to create {}", temporary.display()))?;
    if let Err(error) = file.write_all(bytes).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error).context("failed to write temporary project profile");
    }
    if let Err(error) = file.sync_all().await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error).context("failed to sync temporary project profile");
    }
    drop(file);
    #[cfg(windows)]
    if replace && tokio::fs::try_exists(path).await? {
        tokio::fs::remove_file(path)
            .await
            .with_context(|| format!("failed to replace {}", path.display()))?;
    }
    #[cfg(not(windows))]
    let _ = replace;
    if let Err(error) = tokio::fs::rename(&temporary, path).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error).with_context(|| format!("failed to publish {}", path.display()));
    }
    Ok(())
}

fn monotonic_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn relative_root_reference(root: &Path, config_parent: &Path) -> Result<String> {
    let relative = config_parent
        .strip_prefix(root)
        .context("project profile directory is outside the project root")?;
    let depth = relative
        .components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .count();
    Ok(if depth == 0 {
        ".".to_string()
    } else {
        std::iter::repeat_n("..", depth)
            .collect::<Vec<_>>()
            .join("/")
    })
}

fn relative_config<'a>(root: &'a Path, config: &'a Path) -> Result<&'a Path> {
    config
        .strip_prefix(root)
        .context("generated project profile is outside the project root")
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
