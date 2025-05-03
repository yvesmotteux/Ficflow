use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn test_add_fanfiction() {
    let mut cmd = Command::cargo_bin("ficflow").unwrap();
    cmd.arg("add").arg("12345");
    cmd.assert()
        .success()
        .stdout(contains("Adding fanfiction with ID: 12345"));
}

#[test]
fn test_add_fanfiction_missing_id() {
    let mut cmd = Command::cargo_bin("ficflow").unwrap();
    cmd.arg("add");
    cmd.assert()
        .failure()
        .stderr(contains("error: the following required arguments were not provided:"));
}

#[test]
fn test_delete_fanfiction() {
    let mut cmd = Command::cargo_bin("ficflow").unwrap();
    cmd.arg("delete").arg("12345");
    cmd.assert()
        .success()
        .stdout(contains("Deleting fanfiction with ID: 12345"));
}

#[test]
fn test_delete_fanfiction_missing_id() {
    let mut cmd = Command::cargo_bin("ficflow").unwrap();
    cmd.arg("delete");
    cmd.assert()
        .failure()
        .stderr(contains("error: the following required arguments were not provided:"));
}

#[test]
fn test_list_fanfictions() {
    let mut cmd = Command::cargo_bin("ficflow").unwrap();
    cmd.arg("list");
    cmd.assert()
        .success()
        .stdout(contains("Listing all fanfictions"));
}