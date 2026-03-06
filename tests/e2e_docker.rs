#![cfg(feature = "integration-tests")]

mod common;

use std::collections::HashMap;

use bollard::container::ListContainersOptions;
use common::{TestGrove, connect_docker, connect_postgres};

// ---------------------------------------------------------------------------
// T-021: Full lifecycle tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_lifecycle_install_up_env_status_down() {
    let grove = TestGrove::new();

    // install postgres
    grove.cmd().args(["install", "postgres"]).assert().success();

    // up postgres
    grove.cmd().args(["up", "postgres"]).assert().success();

    // env -- parse and validate
    let env_out = grove.cmd().arg("env").assert().success();
    let stdout = String::from_utf8(env_out.get_output().stdout.clone()).unwrap();
    let env = TestGrove::parse_env_output(&stdout);

    let pgport = env.get("PGPORT").expect("PGPORT missing");
    let port: u16 = pgport.parse().expect("PGPORT not a valid port");
    assert!(port > 0, "PGPORT should be a valid port number");

    let db_url = env.get("DATABASE_URL").expect("DATABASE_URL missing");
    assert!(
        db_url.contains(&format!("localhost:{port}")),
        "DATABASE_URL should contain localhost:{port}, got: {db_url}"
    );

    assert_eq!(env.get("PGHOST").map(|s| s.as_str()), Some("localhost"));
    assert_eq!(env.get("PGUSER").map(|s| s.as_str()), Some("dev"));
    assert_eq!(env.get("PGPASSWORD").map(|s| s.as_str()), Some("dev"));
    assert_eq!(env.get("PGDATABASE").map(|s| s.as_str()), Some("myapp_dev"));

    // No line should start with "export "
    for line in stdout.lines() {
        assert!(
            !line.starts_with("export "),
            "env output should not have export prefix: {line}"
        );
    }

    // status -- should show postgres as running
    let status_out = grove.cmd().arg("status").assert().success();
    let status_stdout = String::from_utf8(status_out.get_output().stdout.clone()).unwrap();
    assert!(
        status_stdout.contains("postgres"),
        "status should list postgres"
    );
    assert!(
        status_stdout.to_lowercase().contains("running"),
        "status should show running"
    );

    // down
    grove.cmd().arg("down").assert().success();

    // status after down
    let status_out2 = grove.cmd().arg("status").assert().success();
    let status_stdout2 = String::from_utf8(status_out2.get_output().stdout.clone()).unwrap();
    assert!(
        status_stdout2.contains("No running services."),
        "status after down should show no running services"
    );

    // data directory should still exist
    let data_dir = grove.store_path().join("data").join("postgres");
    assert!(
        data_dir.exists(),
        "data directory should persist after down: {data_dir:?}"
    );
}

#[tokio::test]
async fn idempotent_up_only_one_container() {
    let grove = TestGrove::new();

    grove.cmd().args(["up", "postgres"]).assert().success();
    grove.cmd().args(["up", "postgres"]).assert().success();

    // Use bollard to verify exactly one container with our prefix
    let docker = connect_docker().expect("Docker must be available");
    let prefix = format!("grov-{}-postgres", grove.grove_prefix);
    let filters: HashMap<String, Vec<String>> = [("name".to_string(), vec![prefix.clone()])].into();
    let opts = ListContainersOptions {
        all: true,
        filters,
        ..Default::default()
    };
    let containers = docker
        .list_containers(Some(opts))
        .await
        .expect("list containers");

    assert_eq!(
        containers.len(),
        1,
        "idempotent up should result in exactly 1 container, found {}",
        containers.len()
    );

    grove.cmd().arg("down").assert().success();
}

#[test]
fn unknown_service_exits_with_code_2() {
    let grove = TestGrove::new();

    grove
        .cmd()
        .args(["up", "nonexistent"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("unknown service"));

    grove
        .cmd()
        .args(["install", "nonexistent"])
        .assert()
        .code(2);

    grove.cmd().args(["down", "nonexistent"]).assert().code(2);
}

#[test]
fn success_exits_with_code_0() {
    let grove = TestGrove::new();

    // status with no services should succeed
    grove.cmd().arg("status").assert().success();

    // env with no services should succeed
    grove.cmd().arg("env").assert().success();
}

#[test]
fn env_output_format_parseable() {
    let grove = TestGrove::new();

    grove.cmd().args(["up", "postgres"]).assert().success();

    let env_out = grove.cmd().arg("env").assert().success();
    let stdout = String::from_utf8(env_out.get_output().stdout.clone()).unwrap();

    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        // No "export " prefix
        assert!(
            !line.starts_with("export "),
            "line should not start with 'export ': {line}"
        );
        // Must be KEY=VALUE
        assert!(
            line.contains('='),
            "line should be KEY=VALUE format: {line}"
        );
        let key = line.split('=').next().unwrap();
        // Key should be valid env var name: uppercase letters, digits, underscore
        assert!(
            !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
            "key should be valid env var name (A-Z, 0-9, _): '{key}'"
        );
    }

    grove.cmd().arg("down").assert().success();
}

// ---------------------------------------------------------------------------
// Clean command tests
// ---------------------------------------------------------------------------

#[test]
fn clean_removes_data_directory() {
    let grove = TestGrove::new();

    // Start and stop minio to create data
    // (minio doesn't create root-owned files like postgres does)
    grove.cmd().args(["up", "minio"]).assert().success();
    grove.cmd().arg("down").assert().success();

    // Verify data dir exists after down
    let data_dir = grove.store_path().join("data").join("minio");
    assert!(data_dir.exists(), "data directory should exist after down");

    // Clean should remove it
    grove.cmd().arg("clean").assert().success();

    let store_path = grove.store_path();
    assert!(
        !store_path.exists(),
        "store directory should be removed after clean: {store_path:?}"
    );
}

#[tokio::test]
async fn clean_stops_running_services_and_removes_data() {
    let grove = TestGrove::new();

    // Start minio (avoids root-owned files that postgres creates)
    grove.cmd().args(["up", "minio"]).assert().success();

    // Verify it's running
    let status_out = grove.cmd().arg("status").assert().success();
    let status_stdout = String::from_utf8(status_out.get_output().stdout.clone()).unwrap();
    assert!(status_stdout.to_lowercase().contains("running"));

    // Get container info before clean
    let docker = connect_docker().expect("Docker must be available");
    let prefix = format!("grov-{}-minio", grove.grove_prefix);
    let filters: HashMap<String, Vec<String>> = [("name".to_string(), vec![prefix.clone()])].into();
    let opts = ListContainersOptions {
        filters,
        ..Default::default()
    };
    let containers_before = docker
        .list_containers(Some(opts))
        .await
        .expect("list containers");
    assert_eq!(containers_before.len(), 1, "container should be running");

    // Clean should stop services and remove data
    grove.cmd().arg("clean").assert().success();

    // Verify container is gone
    let filters: HashMap<String, Vec<String>> = [("name".to_string(), vec![prefix.clone()])].into();
    let opts = ListContainersOptions {
        all: true,
        filters,
        ..Default::default()
    };
    let containers_after = docker
        .list_containers(Some(opts))
        .await
        .expect("list containers");
    assert_eq!(
        containers_after.len(),
        0,
        "container should be removed after clean"
    );

    // Verify store directory is gone
    let store_path = grove.store_path();
    assert!(
        !store_path.exists(),
        "store directory should be removed after clean"
    );
}

#[test]
fn clean_succeeds_with_no_services() {
    let grove = TestGrove::new();

    // Clean with nothing running should succeed
    grove.cmd().arg("clean").assert().success();
}

// ---------------------------------------------------------------------------
// T-026: Cross-grove isolation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_grove_isolation() {
    let grove_a = TestGrove::new();
    let grove_b = TestGrove::new();

    // Start postgres in both groves
    grove_a.cmd().args(["up", "postgres"]).assert().success();
    grove_b.cmd().args(["up", "postgres"]).assert().success();

    // Parse env from both -- ports should differ
    let env_a_out = grove_a.cmd().arg("env").assert().success();
    let env_b_out = grove_b.cmd().arg("env").assert().success();
    let env_a = TestGrove::parse_env_output(
        &String::from_utf8(env_a_out.get_output().stdout.clone()).unwrap(),
    );
    let env_b = TestGrove::parse_env_output(
        &String::from_utf8(env_b_out.get_output().stdout.clone()).unwrap(),
    );

    let port_a = env_a.get("PGPORT").expect("grove A PGPORT");
    let port_b = env_b.get("PGPORT").expect("grove B PGPORT");
    assert_ne!(
        port_a, port_b,
        "different groves should get different ports"
    );

    // Verify both containers are running via bollard
    let docker = connect_docker().expect("Docker must be available");
    for prefix in [&grove_a.grove_prefix, &grove_b.grove_prefix] {
        let name_filter = format!("grov-{}-postgres", prefix);
        let filters: HashMap<String, Vec<String>> =
            [("name".to_string(), vec![name_filter])].into();
        let opts = ListContainersOptions {
            filters,
            ..Default::default()
        };
        let containers = docker
            .list_containers(Some(opts))
            .await
            .expect("list containers");
        assert_eq!(
            containers.len(),
            1,
            "expected 1 container for prefix {prefix}"
        );
    }

    // Down grove A; grove B should still be running
    grove_a.cmd().arg("down").assert().success();

    // Verify grove A's container is gone
    let name_a = format!("grov-{}-postgres", grove_a.grove_prefix);
    let filters_a: HashMap<String, Vec<String>> = [("name".to_string(), vec![name_a])].into();
    let opts_a = ListContainersOptions {
        all: true,
        filters: filters_a,
        ..Default::default()
    };
    let containers_a = docker
        .list_containers(Some(opts_a))
        .await
        .expect("list containers");
    assert_eq!(
        containers_a.len(),
        0,
        "grove A container should be gone after down"
    );

    // Verify grove B's container is still running
    let name_b = format!("grov-{}-postgres", grove_b.grove_prefix);
    let filters_b: HashMap<String, Vec<String>> = [("name".to_string(), vec![name_b])].into();
    let opts_b = ListContainersOptions {
        filters: filters_b,
        ..Default::default()
    };
    let containers_b = docker
        .list_containers(Some(opts_b))
        .await
        .expect("list containers");
    assert_eq!(
        containers_b.len(),
        1,
        "grove B container should still be running"
    );

    // Also verify via grov status
    let status_b = grove_b.cmd().arg("status").assert().success();
    let status_stdout = String::from_utf8(status_b.get_output().stdout.clone()).unwrap();
    assert!(
        status_stdout.contains("postgres"),
        "grove B status should still show postgres"
    );

    grove_b.cmd().arg("down").assert().success();
}

// ---------------------------------------------------------------------------
// T-027: Data persistence across restart
// ---------------------------------------------------------------------------

#[tokio::test]
async fn data_persists_across_restart() {
    let grove = TestGrove::new();

    // First run: start postgres, create table, insert data
    grove.cmd().args(["up", "postgres"]).assert().success();

    let env_out = grove.cmd().arg("env").assert().success();
    let env = TestGrove::parse_env_output(
        &String::from_utf8(env_out.get_output().stdout.clone()).unwrap(),
    );
    let port: u16 = env
        .get("PGPORT")
        .expect("PGPORT")
        .parse()
        .expect("valid port");

    // Connect and create table + insert row
    let (client, conn_handle) = connect_postgres(port).await;
    client
        .execute(
            "CREATE TABLE e2e_test (id SERIAL PRIMARY KEY, value TEXT NOT NULL)",
            &[],
        )
        .await
        .expect("CREATE TABLE");
    client
        .execute(
            "INSERT INTO e2e_test (value) VALUES ($1)",
            &[&"persistence_check"],
        )
        .await
        .expect("INSERT");

    // Drop client before stopping
    drop(client);
    conn_handle.abort();

    grove.cmd().arg("down").assert().success();

    // Second run: start again (same grove = same data dir), verify data
    grove.cmd().args(["up", "postgres"]).assert().success();

    let env_out2 = grove.cmd().arg("env").assert().success();
    let env2 = TestGrove::parse_env_output(
        &String::from_utf8(env_out2.get_output().stdout.clone()).unwrap(),
    );
    let port2: u16 = env2
        .get("PGPORT")
        .expect("PGPORT")
        .parse()
        .expect("valid port");

    let (client2, conn_handle2) = connect_postgres(port2).await;
    let rows = client2
        .query("SELECT value FROM e2e_test WHERE id = 1", &[])
        .await
        .expect("SELECT");

    assert_eq!(rows.len(), 1, "should find exactly one row");
    let value: &str = rows[0].get(0);
    assert_eq!(
        value, "persistence_check",
        "data should persist across restart"
    );

    drop(client2);
    conn_handle2.abort();

    grove.cmd().arg("down").assert().success();
}
