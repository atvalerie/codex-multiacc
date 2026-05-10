use std::path::Path;

use anyhow::Result;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

fn write_file_auth_config(codex_home: &Path) -> Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        "cli_auth_credentials_store = \"file\"\n",
    )?;
    Ok(())
}

fn read_auth_json(codex_home: &Path) -> Result<Value> {
    let auth_json = std::fs::read_to_string(codex_home.join("auth.json"))?;
    Ok(serde_json::from_str(&auth_json)?)
}

#[test]
fn login_with_api_key_reads_stdin_and_writes_auth_json() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_file_auth_config(codex_home.path())?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args([
        "-c",
        "forced_login_method=\"api\"",
        "login",
        "--with-api-key",
    ])
    .write_stdin("sk-test\n")
    .assert()
    .success()
    .stderr(contains("Successfully logged in"));

    let auth = read_auth_json(codex_home.path())?;
    assert_eq!(auth["OPENAI_API_KEY"], "sk-test");
    assert!(auth.get("tokens").is_none());
    assert!(auth.get("agent_identity").is_none());

    Ok(())
}

#[test]
fn login_accounts_lists_switches_and_removes_stored_accounts() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_file_auth_config(codex_home.path())?;

    let mut first_login = codex_command(codex_home.path())?;
    first_login
        .args(["login", "--with-api-key"])
        .write_stdin("sk-first\n")
        .assert()
        .success();
    let first_auth = read_auth_json(codex_home.path())?;
    let first_account_id = first_auth["active_account_id"]
        .as_str()
        .expect("first login should set active account")
        .to_string();

    let mut second_login = codex_command(codex_home.path())?;
    second_login
        .args(["login", "--with-api-key"])
        .write_stdin("sk-second\n")
        .assert()
        .success();

    let auth = read_auth_json(codex_home.path())?;
    assert_eq!(auth["OPENAI_API_KEY"], "sk-second");
    assert_eq!(
        auth["accounts"]
            .as_object()
            .expect("accounts should be stored")
            .len(),
        2
    );

    let mut list = codex_command(codex_home.path())?;
    list.args(["login", "list"])
        .assert()
        .success()
        .stderr(contains("[apikey] API key"));

    let mut switch = codex_command(codex_home.path())?;
    switch
        .args(["login", "use", &first_account_id])
        .assert()
        .success()
        .stderr(contains(format!("Switched to account {first_account_id}")));

    let switched_auth = read_auth_json(codex_home.path())?;
    assert_eq!(switched_auth["OPENAI_API_KEY"], "sk-first");

    let mut remove = codex_command(codex_home.path())?;
    remove
        .args(["login", "logout", &first_account_id])
        .assert()
        .success()
        .stderr(contains(format!("Removed account {first_account_id}")));

    let remaining_auth = read_auth_json(codex_home.path())?;
    assert_eq!(
        remaining_auth["accounts"]
            .as_object()
            .expect("remaining account should be stored")
            .len(),
        1
    );
    assert_eq!(remaining_auth["OPENAI_API_KEY"], "sk-second");

    Ok(())
}

#[test]
fn login_with_access_token_rejects_invalid_jwt() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_file_auth_config(codex_home.path())?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args(["login", "--with-access-token"])
        .write_stdin("not-a-jwt\n")
        .assert()
        .failure()
        .stderr(contains("Error logging in with access token"));

    Ok(())
}
