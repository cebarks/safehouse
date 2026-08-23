use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use bollard::container::LogOutput;
use bollard::models::{ContainerCreateBody, HostConfig, PortBinding};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, LogsOptionsBuilder, RemoveContainerOptionsBuilder,
    StopContainerOptionsBuilder,
};
use bollard::Docker;
use futures_util::StreamExt;

use crate::config::SafehouseConfig;
use crate::pz::case_fix::fix_case;

/// Default container name managed by safehouse.
pub const CONTAINER_NAME: &str = "safehouse-pz";

/// Default image name (built from the repo's Containerfile).
pub const IMAGE_NAME: &str = "ghcr.io/cebarks/safehouse-pz:latest";

/// Connect to the local podman/docker daemon.
pub async fn connect() -> Result<Docker> {
    Docker::connect_with_podman_defaults()
        .context("failed to connect to podman/docker — is the socket active?")
}

/// Ensure the safehouse-pz image exists locally, pulling from GHCR if needed.
pub async fn ensure_image(docker: &Docker) -> Result<()> {
    if docker.inspect_image(IMAGE_NAME).await.is_ok() {
        return Ok(());
    }

    println!("Pulling {IMAGE_NAME}...");
    use bollard::query_parameters::CreateImageOptionsBuilder;
    use futures_util::TryStreamExt;

    let opts = CreateImageOptionsBuilder::default()
        .from_image(IMAGE_NAME)
        .build();

    docker
        .create_image(Some(opts), None, None)
        .try_collect::<Vec<_>>()
        .await
        .with_context(|| format!(
            "Failed to pull '{IMAGE_NAME}'.\n\
             You can also build locally: podman build -t safehouse-pz -f Containerfile ."
        ))?;

    Ok(())
}

/// Check if the safehouse container is currently running.
pub async fn is_running(docker: &Docker) -> bool {
    match docker.inspect_container(CONTAINER_NAME, None).await {
        Ok(info) => info
            .state
            .and_then(|s| s.running)
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Create and start the PZ server container.
pub async fn create_and_start(docker: &Docker, config: &SafehouseConfig) -> Result<()> {
    // Fix case-sensitivity issues: create lowercase symlinks so PZ's
    // lowercased path lookups resolve on Linux. Must run BEFORE
    // create_container because the :Z bind mount triggers SELinux
    // relabeling — symlinks created afterward would lack container_file_t.
    match fix_case(&config.server_install_dir) {
        Ok(result) => {
            if result.symlinks_created > 0 || result.symlinks_cleaned > 0 {
                tracing::info!(
                    created = result.symlinks_created,
                    cleaned = result.symlinks_cleaned,
                    failures = result.failures,
                    "case-fix scan complete",
                );
            }
            for w in &result.warnings {
                tracing::warn!("case-fix: {w}");
            }
        }
        Err(e) => tracing::warn!("case-fix scan failed, continuing: {e}"),
    }

    // Clean up any leftover stopped container with the same name
    let _ = docker
        .remove_container(
            CONTAINER_NAME,
            Some(
                RemoveContainerOptionsBuilder::default()
                    .force(true)
                    .build(),
            ),
        )
        .await;

    // Volume mounts
    let server_dir = config.server_install_dir.to_string_lossy();
    let zomboid_dir = config.zomboid_dir().to_string_lossy().to_string();

    let binds = vec![
        format!("{server_dir}:/server:Z"),
        format!("{zomboid_dir}:/zomboid:Z"),
    ];

    // Port bindings
    let mut port_bindings: HashMap<String, Option<Vec<PortBinding>>> = HashMap::new();

    // Game ports
    port_bindings.insert(
        "16261/udp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("0.0.0.0".to_string()),
            host_port: Some("16261".to_string()),
        }]),
    );
    port_bindings.insert(
        "16262/udp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("0.0.0.0".to_string()),
            host_port: Some("16262".to_string()),
        }]),
    );

    // RCON port
    port_bindings.insert(
        "27015/tcp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("127.0.0.1".to_string()),
            host_port: Some(config.rcon_port.to_string()),
        }]),
    );

    let host_config = HostConfig {
        binds: Some(binds),
        port_bindings: Some(port_bindings),
        ..Default::default()
    };

    // Container args:
    //   -cachedir=/zomboid  — PZ uses Java user.home (=/root) by default, not $HOME;
    //                         -cachedir overrides the data directory so PZ reads our
    //                         volume-mounted ~/Zomboid at /zomboid.
    //   -servername <name>
    //   -adminpassword <pass> (optional)
    let mut cmd = vec![
        "-cachedir=/zomboid".to_string(),
        "-servername".to_string(),
        config.server_name.clone(),
    ];
    if !config.rcon_password.is_empty() {
        cmd.push("-adminpassword".to_string());
        cmd.push(config.rcon_password.clone());
    }

    let container_config = ContainerCreateBody {
        image: Some(IMAGE_NAME.to_string()),
        cmd: Some(cmd),
        host_config: Some(host_config),
        ..Default::default()
    };

    let options = CreateContainerOptionsBuilder::default()
        .name(CONTAINER_NAME)
        .build();

    docker
        .create_container(Some(options), container_config)
        .await
        .context("failed to create container")?;

    docker
        .start_container(CONTAINER_NAME, None::<bollard::query_parameters::StartContainerOptions>)
        .await
        .context("failed to start container")?;

    Ok(())
}

/// Stop the container gracefully.
pub async fn stop(docker: &Docker, timeout_secs: i32) -> Result<()> {
    let options = StopContainerOptionsBuilder::default()
        .t(timeout_secs)
        .build();

    match docker.stop_container(CONTAINER_NAME, Some(options)).await {
        Ok(_) => Ok(()),
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 304, ..
        }) => {
            // 304 = container already stopped
            Ok(())
        }
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => {
            // Container doesn't exist
            Ok(())
        }
        Err(e) => Err(e).context("failed to stop container"),
    }
}

/// Remove the container.
pub async fn remove(docker: &Docker) -> Result<()> {
    let options = RemoveContainerOptionsBuilder::default()
        .force(true)
        .build();

    match docker.remove_container(CONTAINER_NAME, Some(options)).await {
        Ok(_) => Ok(()),
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => Ok(()),
        Err(e) => Err(e).context("failed to remove container"),
    }
}

/// Stream container logs to stdout.
pub async fn stream_logs(docker: &Docker, follow: bool, tail: usize) -> Result<()> {
    stream_container_logs(docker, CONTAINER_NAME, follow, tail).await
}

async fn stream_container_logs(
    docker: &Docker,
    container: &str,
    follow: bool,
    tail: usize,
) -> Result<()> {
    let options = LogsOptionsBuilder::default()
        .stdout(true)
        .stderr(true)
        .follow(follow)
        .tail(tail.to_string().as_str())
        .build();

    let mut stream = docker.logs(container, Some(options));

    while let Some(result) = stream.next().await {
        match result {
            Ok(output) => match output {
                LogOutput::StdOut { message } | LogOutput::StdErr { message } => {
                    let text = String::from_utf8_lossy(&message);
                    print!("{text}");
                }
                _ => {}
            },
            Err(e) => {
                tracing::warn!("Log stream error: {e}");
                break;
            }
        }
    }

    Ok(())
}

/// Run steamcmd inside the container to install/update PZ.
/// Maximum number of SteamCMD install attempts. SteamCMD intermittently fails
/// with "Missing configuration" — a known bug where its self-update or app
/// metadata download doesn't complete on the first try.
const STEAMCMD_MAX_ATTEMPTS: u32 = 3;

pub async fn run_steamcmd_install(docker: &Docker, config: &SafehouseConfig) -> Result<()> {
    println!("Installing Project Zomboid dedicated server via SteamCMD...");

    let server_dir = config.server_install_dir.to_string_lossy().to_string();
    let mut last_exit: i64 = -1;

    for attempt in 1..=STEAMCMD_MAX_ATTEMPTS {
        // Clean up any leftover container
        let _ = docker
            .remove_container(
                "safehouse-setup",
                Some(
                    RemoveContainerOptionsBuilder::default()
                        .force(true)
                        .build(),
                ),
            )
            .await;

        let host_config = HostConfig {
            binds: Some(vec![format!("{server_dir}:/server:Z")]),
            ..Default::default()
        };

        let container_config = ContainerCreateBody {
            image: Some(IMAGE_NAME.to_string()),
            entrypoint: Some(vec!["steamcmd.sh".to_string()]),
            cmd: Some(vec![
                "+force_install_dir".to_string(),
                "/server".to_string(),
                "+login".to_string(),
                "anonymous".to_string(),
                "+app_update".to_string(),
                "380870".to_string(),
                "validate".to_string(),
                "+quit".to_string(),
            ]),
            host_config: Some(host_config),
            ..Default::default()
        };

        let options = CreateContainerOptionsBuilder::default()
            .name("safehouse-setup")
            .build();

        docker
            .create_container(Some(options), container_config)
            .await
            .context("failed to create setup container")?;

        docker
            .start_container("safehouse-setup", None::<bollard::query_parameters::StartContainerOptions>)
            .await
            .context("failed to start setup container")?;

        if attempt > 1 {
            println!("SteamCMD attempt {attempt}/{STEAMCMD_MAX_ATTEMPTS}...");
        }
        stream_container_logs(docker, "safehouse-setup", true, 0).await.ok();

        // Wait for the container to fully stop, then grab exit code
        let mut wait_stream = docker.wait_container("safehouse-setup", None::<bollard::query_parameters::WaitContainerOptions>);
        last_exit = match wait_stream.next().await {
            Some(Ok(resp)) => resp.status_code,
            Some(Err(e)) => {
                tracing::debug!("wait_container error (may already be stopped): {e}");
                docker
                    .inspect_container("safehouse-setup", None)
                    .await?
                    .state
                    .and_then(|s| s.exit_code)
                    .unwrap_or(-1) as i64
            }
            None => -1,
        };

        // Cleanup the container between attempts
        let _ = docker
            .remove_container(
                "safehouse-setup",
                Some(
                    RemoveContainerOptionsBuilder::default()
                        .force(true)
                        .build(),
                ),
            )
            .await;

        if last_exit == 0 {
            break;
        }

        if attempt < STEAMCMD_MAX_ATTEMPTS {
            println!(
                "SteamCMD exited with code {last_exit}, retrying ({}/{STEAMCMD_MAX_ATTEMPTS})...",
                attempt + 1,
            );
        }
    }

    if last_exit != 0 {
        bail!("SteamCMD failed after {STEAMCMD_MAX_ATTEMPTS} attempts (last exit code: {last_exit})");
    }

    println!("PZ server installed.");

    // Fix case-sensitivity issues on freshly downloaded files.
    match fix_case(&config.server_install_dir) {
        Ok(result) if result.symlinks_created > 0 => {
            println!(
                "Case-fix: created {} lowercase symlinks, cleaned {} dangling",
                result.symlinks_created, result.symlinks_cleaned,
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("case-fix scan failed after install: {e}"),
    }

    Ok(())
}
