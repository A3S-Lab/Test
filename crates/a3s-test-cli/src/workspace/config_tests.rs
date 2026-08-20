use std::fs;
use std::path::Path;

use super::*;

#[tokio::test]
async fn profile_symlinks_are_rejected() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let temp = project_fixture();
        let target = temp.path().join("profile-target.acl");
        fs::write(&target, valid_profile()).expect("target profile");
        symlink(&target, temp.path().join(".a3s-test/project.acl")).expect("profile symlink");

        let error = load(temp.path(), Path::new(".a3s-test/project.acl"))
            .await
            .expect_err("profile symlink must fail");

        assert!(error.to_string().contains("regular non-link"));
    }
}

#[tokio::test]
async fn profile_paths_cannot_escape_the_project_root() {
    let project = project_fixture();
    let outside = tempfile::tempdir().expect("outside tempdir");
    let outside_profile = outside.path().join("project.acl");
    fs::write(&outside_profile, valid_profile()).expect("outside profile");

    let error = load(project.path(), &outside_profile)
        .await
        .expect_err("outside profile must fail");

    assert!(error.to_string().contains("inside the project root"));
}

#[tokio::test]
async fn profile_root_cannot_expand_the_admitted_project_scope() {
    let parent = tempfile::tempdir().expect("parent tempdir");
    let project = parent.path().join("project");
    fs::create_dir(&project).expect("project root");
    fs::create_dir(project.join(".a3s-test")).expect("profile directory");
    write_profile(
        &project,
        &valid_profile().replace("root = \"..\"", "root = \"../..\""),
    );

    let error = load(&project, Path::new(".a3s-test/project.acl"))
        .await
        .expect_err("expanded project root must fail");

    assert!(error.to_string().contains("admitted by --root"));
}

#[tokio::test]
async fn development_commands_may_have_no_arguments() {
    let temp = project_fixture();
    write_profile(
        temp.path(),
        &valid_profile().replace("args = [\"run\", \"dev\"]", "args = []"),
    );

    let profile = load(temp.path(), Path::new(".a3s-test/project.acl"))
        .await
        .expect("argument-free development command");

    assert!(profile.dev_server.arguments.is_empty());
}

#[tokio::test]
async fn unknown_attributes_are_rejected() {
    let temp = project_fixture();
    write_profile(
        temp.path(),
        &valid_profile().replace("version = 1", "version = 1\n  mystery = true"),
    );

    let error = load(temp.path(), Path::new(".a3s-test/project.acl"))
        .await
        .expect_err("unknown attribute must fail");

    assert!(error.to_string().contains("unsupported project attribute"));
}

#[tokio::test]
async fn development_timeouts_are_strictly_bounded() {
    for (field, invalid) in [
        ("startup_timeout_ms = 120000", "startup_timeout_ms = 0"),
        ("startup_timeout_ms = 120000", "startup_timeout_ms = 600001"),
        ("cleanup_timeout_ms = 10000", "cleanup_timeout_ms = 0"),
        ("cleanup_timeout_ms = 10000", "cleanup_timeout_ms = 60001"),
    ] {
        let temp = project_fixture();
        write_profile(temp.path(), &valid_profile().replace(field, invalid));

        let error = load(temp.path(), Path::new(".a3s-test/project.acl"))
            .await
            .expect_err("out-of-range timeout must fail");

        assert!(error.to_string().contains("must be between 1"), "{error:#}");
    }
}

#[tokio::test]
async fn working_directories_cannot_traverse_outside_the_project() {
    let temp = project_fixture();
    write_profile(
        temp.path(),
        &valid_profile().replace(
            "working_directory = \".\"",
            "working_directory = \"../outside\"",
        ),
    );

    let error = load(temp.path(), Path::new(".a3s-test/project.acl"))
        .await
        .expect_err("escaping working directory must fail");

    assert!(error.to_string().contains("must stay inside"));
}

fn project_fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(temp.path().join(".a3s-test")).expect("profile directory");
    temp
}

fn write_profile(root: &Path, source: &str) {
    fs::write(root.join(".a3s-test/project.acl"), source).expect("project profile");
}

fn valid_profile() -> String {
    r#"project "fixture" {
  version = 1
  root = ".."

  dev_server {
    executable = "npm"
    args = ["run", "dev"]
    working_directory = "."
    url = "http://127.0.0.1:5173/"
    startup_timeout_ms = 120000
    cleanup_timeout_ms = 10000
  }

  browser {
    driver = "a3s"
    session = "dev"
    headed = true
    command_timeout_ms = 25000
    idle_timeout_ms = 300000
  }

  testkit {
    required = true
  }
}
"#
    .to_string()
}
