use std::path::PathBuf;

pub(crate) fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
pub(crate) fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};

    const VERBATIM_PREFIX: &[u16] = &[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
    const UNC_PREFIX: &[u16] = &[b'U' as u16, b'N' as u16, b'C' as u16, b'\\' as u16];

    let wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
    if !wide.starts_with(VERBATIM_PREFIX) {
        return path;
    }
    if wide[VERBATIM_PREFIX.len()..].starts_with(UNC_PREFIX) {
        let mut normalized = vec![b'\\' as u16, b'\\' as u16];
        normalized.extend_from_slice(&wide[VERBATIM_PREFIX.len() + UNC_PREFIX.len()..]);
        return PathBuf::from(OsString::from_wide(&normalized));
    }
    PathBuf::from(OsString::from_wide(&wide[VERBATIM_PREFIX.len()..]))
}

#[cfg(not(windows))]
pub(crate) fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    path
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::*;

    #[cfg(windows)]
    #[test]
    fn normalizes_windows_verbatim_drive_and_unc_paths() {
        assert_eq!(
            normalize_canonical_path(PathBuf::from(r"\\?\C:\work\artifact")),
            PathBuf::from(r"C:\work\artifact")
        );
        assert_eq!(
            normalize_canonical_path(PathBuf::from(r"\\?\UNC\server\share\artifact")),
            PathBuf::from(r"\\server\share\artifact")
        );
    }
}
