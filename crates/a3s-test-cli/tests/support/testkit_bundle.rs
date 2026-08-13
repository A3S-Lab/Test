use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

const ESBUILD_SCRIPT: &str = "const esbuild=require(process.argv[1]);esbuild.buildSync({entryPoints:[process.argv[2]],bundle:true,format:'esm',platform:'browser',target:'es2022',outfile:process.argv[3]});";

pub fn bundle_browser_fixture(context: &str) -> (TempDir, Vec<u8>) {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crate workspace root")
        .to_path_buf();
    let package_root = crate_root.join("packages/testkit");
    let esbuild = package_root.join("node_modules/esbuild/lib/main.js");
    assert!(
        esbuild.is_file(),
        "run `npm install` in packages/testkit before this E2E"
    );
    let bundle_workspace = tempfile::tempdir().expect("temporary TestKit bundle workspace");
    let bundle_path = bundle_workspace.path().join("testkit.js");
    let entry = package_root.join("src/browser-fixture.tsx");
    let output = Command::new("node")
        .arg("-e")
        .arg(ESBUILD_SCRIPT)
        .arg(&esbuild)
        .arg(&entry)
        .arg(&bundle_path)
        .current_dir(&package_root)
        .output()
        .unwrap_or_else(|error| panic!("{context}: failed to launch Node bundler: {error}"));
    assert!(
        output.status.success(),
        "{context} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let bundle = std::fs::read(&bundle_path)
        .unwrap_or_else(|error| panic!("{context}: failed to read bundle: {error}"));
    (bundle_workspace, bundle)
}
