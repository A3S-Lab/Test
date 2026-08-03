use std::path::{Path, PathBuf};

use a3s_test_core::DriverError;

use crate::path_security::{is_link_like, normalize_canonical_path};

#[derive(Clone, Debug)]
pub(crate) struct RuntimeDirectory {
    path: PathBuf,
    identity: RuntimeIdentity,
}

impl RuntimeDirectory {
    pub(crate) async fn bind_or_create(path: &Path) -> Result<Self, DriverError> {
        validate_absolute(path)?;
        match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) => validate_initial_entry(&metadata)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tokio::fs::create_dir_all(path).await.map_err(|error| {
                    DriverError::new(
                        "test.driver.web.runtime_create_failed",
                        format!("failed to create browser runtime directory: {error}"),
                    )
                })?;
            }
            Err(error) => {
                return Err(DriverError::new(
                    "test.driver.web.runtime_path_invalid",
                    format!("failed to inspect browser runtime directory: {error}"),
                ));
            }
        }
        Self::bind_existing(path).await
    }

    pub(crate) async fn bind_existing(path: &Path) -> Result<Self, DriverError> {
        validate_absolute(path)?;
        let metadata = tokio::fs::symlink_metadata(path).await.map_err(|error| {
            DriverError::new(
                "test.driver.web.runtime_path_invalid",
                format!("failed to inspect browser runtime directory: {error}"),
            )
        })?;
        validate_initial_entry(&metadata)?;
        let canonical = tokio::fs::canonicalize(path)
            .await
            .map(normalize_canonical_path)
            .map_err(|error| {
                DriverError::new(
                    "test.driver.web.runtime_path_invalid",
                    format!("failed to resolve browser runtime directory: {error}"),
                )
            })?;
        let canonical_metadata =
            tokio::fs::symlink_metadata(&canonical)
                .await
                .map_err(|error| {
                    DriverError::new(
                        "test.driver.web.runtime_path_invalid",
                        format!("failed to inspect resolved browser runtime directory: {error}"),
                    )
                })?;
        validate_initial_entry(&canonical_metadata)?;
        let identity = capture_identity(canonical.clone(), canonical_metadata)
            .await
            .map_err(|error| {
                DriverError::new(
                    "test.driver.web.runtime_path_invalid",
                    format!("failed to bind browser runtime directory: {error}"),
                )
            })?;
        Ok(Self {
            path: canonical,
            identity,
        })
    }

    pub(crate) async fn verify(&self) -> Result<(), DriverError> {
        let metadata = tokio::fs::symlink_metadata(&self.path)
            .await
            .map_err(|error| {
                binding_lost(format!("failed to inspect runtime directory: {error}"))
            })?;
        validate_bound_entry(&metadata)?;
        let canonical = tokio::fs::canonicalize(&self.path)
            .await
            .map(normalize_canonical_path)
            .map_err(|error| {
                binding_lost(format!("failed to resolve runtime directory: {error}"))
            })?;
        validate_canonical_path(self, &canonical)?;
        let identity = capture_identity(canonical, metadata)
            .await
            .map_err(|error| binding_lost(format!("failed to bind runtime directory: {error}")))?;
        validate_identity(self, &identity)
    }

    pub(crate) fn verify_sync(&self) -> Result<(), DriverError> {
        let metadata = std::fs::symlink_metadata(&self.path).map_err(|error| {
            binding_lost(format!("failed to inspect runtime directory: {error}"))
        })?;
        validate_bound_entry(&metadata)?;
        let canonical = std::fs::canonicalize(&self.path)
            .map(normalize_canonical_path)
            .map_err(|error| {
                binding_lost(format!("failed to resolve runtime directory: {error}"))
            })?;
        validate_canonical_path(self, &canonical)?;
        let identity = RuntimeIdentity::from_path(&canonical, &metadata)
            .map_err(|error| binding_lost(format!("failed to bind runtime directory: {error}")))?;
        validate_identity(self, &identity)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(unix)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeIdentity {
    device: u64,
    inode: u64,
}

#[cfg(windows)]
#[derive(Clone, Debug)]
struct RuntimeIdentity {
    volume: u32,
    index: u64,
    _handle: std::sync::Arc<std::fs::File>,
}

#[cfg(windows)]
impl PartialEq for RuntimeIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.volume == other.volume && self.index == other.index
    }
}

#[cfg(windows)]
impl Eq for RuntimeIdentity {}

#[cfg(not(any(unix, windows)))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeIdentity {}

impl RuntimeIdentity {
    fn from_path(path: &Path, metadata: &std::fs::Metadata) -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;

            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }
        #[cfg(windows)]
        {
            let _ = metadata;
            windows_directory_identity(path)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = path;
            let _ = metadata;
            Ok(Self {})
        }
    }
}

#[cfg(windows)]
fn windows_directory_identity(path: &Path) -> std::io::Result<RuntimeIdentity> {
    use std::mem::MaybeUninit;
    use std::os::windows::fs::OpenOptionsExt as _;
    use std::os::windows::io::AsRawHandle as _;

    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const FILE_SHARE_DELETE: u32 = 0x0000_0004;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    let mut information = MaybeUninit::<ByHandleFileInformation>::uninit();
    // SAFETY: `file` owns a valid handle and `information` points to writable,
    // correctly sized storage for the duration of the system call.
    let succeeded =
        unsafe { get_file_information_by_handle(file.as_raw_handle(), information.as_mut_ptr()) };
    if succeeded == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: a successful call initializes the complete output structure.
    let information = unsafe { information.assume_init() };
    Ok(RuntimeIdentity {
        volume: information.volume_serial_number,
        index: (u64::from(information.file_index_high) << 32)
            | u64::from(information.file_index_low),
        _handle: std::sync::Arc::new(file),
    })
}

#[cfg(windows)]
#[repr(C)]
struct FileTime {
    low_date_time: u32,
    high_date_time: u32,
}

#[cfg(windows)]
#[repr(C)]
struct ByHandleFileInformation {
    file_attributes: u32,
    creation_time: FileTime,
    last_access_time: FileTime,
    last_write_time: FileTime,
    volume_serial_number: u32,
    file_size_high: u32,
    file_size_low: u32,
    number_of_links: u32,
    file_index_high: u32,
    file_index_low: u32,
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    #[link_name = "GetFileInformationByHandle"]
    fn get_file_information_by_handle(
        file: std::os::windows::io::RawHandle,
        information: *mut ByHandleFileInformation,
    ) -> i32;
}

async fn capture_identity(
    path: PathBuf,
    metadata: std::fs::Metadata,
) -> std::io::Result<RuntimeIdentity> {
    #[cfg(windows)]
    {
        tokio::task::spawn_blocking(move || RuntimeIdentity::from_path(&path, &metadata))
            .await
            .map_err(std::io::Error::other)?
    }
    #[cfg(not(windows))]
    {
        RuntimeIdentity::from_path(&path, &metadata)
    }
}

fn validate_absolute(path: &Path) -> Result<(), DriverError> {
    if !path.is_absolute() {
        return Err(DriverError::new(
            "test.driver.web.runtime_path_invalid",
            "persistent browser runtime directory must be absolute",
        ));
    }
    Ok(())
}

fn validate_initial_entry(metadata: &std::fs::Metadata) -> Result<(), DriverError> {
    if is_link_like(metadata) || !metadata.is_dir() {
        return Err(DriverError::new(
            "test.driver.web.runtime_path_invalid",
            "browser runtime path is a link or non-directory entry",
        ));
    }
    Ok(())
}

fn validate_bound_entry(metadata: &std::fs::Metadata) -> Result<(), DriverError> {
    if is_link_like(metadata) || !metadata.is_dir() {
        return Err(binding_lost(
            "browser runtime path became a link or non-directory entry",
        ));
    }
    Ok(())
}

fn validate_canonical_path(
    expected: &RuntimeDirectory,
    canonical: &Path,
) -> Result<(), DriverError> {
    if canonical != expected.path {
        return Err(binding_lost(
            "browser runtime directory no longer matches the connected session",
        ));
    }
    Ok(())
}

fn validate_identity(
    expected: &RuntimeDirectory,
    identity: &RuntimeIdentity,
) -> Result<(), DriverError> {
    if identity != &expected.identity {
        return Err(binding_lost(
            "browser runtime directory identity changed after connection",
        ));
    }
    Ok(())
}

fn binding_lost(message: impl Into<String>) -> DriverError {
    DriverError::new("test.driver.web.runtime_binding_lost", message)
}
