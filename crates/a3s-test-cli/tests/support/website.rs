use std::path::PathBuf;
use std::process::Command;

pub fn build_website(context: &str) -> PathBuf {
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crate workspace root")
        .to_path_buf();
    let website_root = repository_root.join("website");
    assert!(
        website_root.join("node_modules").is_dir(),
        "run `npm install` in website before this E2E"
    );
    assert!(
        repository_root
            .join("packages/testkit/node_modules")
            .is_dir(),
        "run `npm install` in packages/testkit before this E2E"
    );

    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let output = Command::new(npm)
        .args(["run", "build"])
        .current_dir(&website_root)
        .env("DOCS_BASE", "/Test/")
        .env("DOCS_ORIGIN", "http://127.0.0.1")
        .output()
        .unwrap_or_else(|error| panic!("{context}: failed to launch website build: {error}"));
    assert!(
        output.status.success(),
        "{context} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let output_root = website_root.join("doc_build");
    assert!(
        output_root.join("index.html").is_file(),
        "{context} did not produce website/doc_build/index.html"
    );
    output_root
}
