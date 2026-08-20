use std::fs;

use super::*;

#[tokio::test]
async fn absolute_config_paths_cannot_use_parent_traversal() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("project");
    fs::create_dir(&root).expect("project root");
    let requested = root.join("nested/../../outside/project.acl");

    let error = init_config_path(&root, &requested)
        .await
        .expect_err("parent traversal must fail");

    assert!(error.to_string().contains("contained path"));
    assert!(!temp.path().join("outside").exists());
}

#[tokio::test]
async fn linked_config_parents_cannot_create_directories_outside_the_project() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("project");
        let outside = temp.path().join("outside");
        fs::create_dir(&root).expect("project root");
        fs::create_dir(&outside).expect("outside directory");
        symlink(&outside, root.join("linked")).expect("directory symlink");

        let error = create_contained_directories(&root, &root.join("linked/nested"))
            .await
            .expect_err("linked parent must fail");

        assert!(error.to_string().contains("regular directory"));
        assert!(!outside.join("nested").exists());
    }
}

#[tokio::test]
async fn contained_config_directories_are_created_component_by_component() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("project");
    fs::create_dir(&root).expect("project root");
    let directory = root.join("config/a3s-test");

    create_contained_directories(&root, &directory)
        .await
        .expect("contained directories");

    assert!(directory.is_dir());
}
