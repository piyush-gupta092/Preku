//! Docker containers (PRD M4) through the local Docker API (named pipe / unix socket).

use std::collections::HashSet;

use bollard::{
    models::ContainerSummaryStateEnum,
    query_parameters::{
        ListContainersOptionsBuilder, RestartContainerOptions, StartContainerOptions,
        StopContainerOptions,
    },
    Docker,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::foundation::errors::{PrekuError, Result};

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    /// `running`, `exited`, `paused`, …
    pub state: String,
    /// Docker's human text, e.g. "Up 3 hours" / "Exited (1) 2 minutes ago".
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DockerState {
    /// False when Docker isn't installed or not running.
    pub available: bool,
    pub containers: Vec<ContainerInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
}

/// `Exited (137) 3 seconds ago` → 137.
pub fn exit_code(status: &str) -> Option<i64> {
    let rest = status.strip_prefix("Exited (")?;
    rest.split(')').next()?.trim().parse().ok()
}

/// Containers that were running and are now exited with a non-zero code, excluding ones
/// the user stopped from Preku (`expected`).
pub fn unexpected_exits(
    before: &[ContainerInfo],
    now: &[ContainerInfo],
    expected: &HashSet<String>,
) -> Vec<(String, i64)> {
    now.iter()
        .filter(|c| c.state == "exited" && !expected.contains(&c.id))
        .filter(|c| before.iter().any(|b| b.id == c.id && b.state == "running"))
        .filter_map(|c| {
            exit_code(&c.status)
                .filter(|code| *code != 0)
                .map(|code| (c.name.clone(), code))
        })
        .collect()
}

fn state_name(state: Option<ContainerSummaryStateEnum>) -> String {
    match state {
        Some(state) => serde_json::to_value(state)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default(),
        None => String::new(),
    }
}

fn docker_err(err: bollard::errors::Error) -> PrekuError {
    PrekuError::Io(std::io::Error::other(err))
}

pub fn connect() -> Result<Docker> {
    Docker::connect_with_local_defaults().map_err(docker_err)
}

pub async fn list(docker: &Docker) -> Result<Vec<ContainerInfo>> {
    let options = ListContainersOptionsBuilder::new().all(true).build();
    let summaries = docker
        .list_containers(Some(options))
        .await
        .map_err(docker_err)?;
    let mut containers: Vec<ContainerInfo> = summaries
        .into_iter()
        .map(|c| ContainerInfo {
            id: c.id.unwrap_or_default(),
            name: c
                .names
                .and_then(|names| names.into_iter().next())
                .map(|n| n.trim_start_matches('/').to_owned())
                .unwrap_or_default(),
            image: c.image.unwrap_or_default(),
            state: state_name(c.state),
            status: c.status.unwrap_or_default(),
        })
        .collect();
    // Running first, then by name, so the list is stable between polls.
    containers
        .sort_by(|a, b| (a.state != "running", &a.name).cmp(&(b.state != "running", &b.name)));
    Ok(containers)
}

pub async fn act(docker: &Docker, id: &str, action: ContainerAction) -> Result<()> {
    match action {
        ContainerAction::Start => {
            docker
                .start_container(id, None::<StartContainerOptions>)
                .await
        }
        ContainerAction::Stop => {
            docker
                .stop_container(id, None::<StopContainerOptions>)
                .await
        }
        ContainerAction::Restart => {
            docker
                .restart_container(id, None::<RestartContainerOptions>)
                .await
        }
    }
    .map_err(docker_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn container(id: &str, state: &str, status: &str) -> ContainerInfo {
        ContainerInfo {
            id: id.into(),
            name: format!("{id}-name"),
            image: "img".into(),
            state: state.into(),
            status: status.into(),
        }
    }

    #[test]
    fn parses_exit_codes() {
        assert_eq!(exit_code("Exited (137) 3 seconds ago"), Some(137));
        assert_eq!(exit_code("Exited (0) 1 hour ago"), Some(0));
        assert_eq!(exit_code("Up 2 hours"), None);
    }

    #[test]
    fn detects_crashes_but_not_clean_or_requested_stops() {
        let before = [
            container("a", "running", "Up"),
            container("b", "running", "Up"),
            container("c", "running", "Up"),
            container("d", "exited", "Exited (1) 1 hour ago"),
        ];
        let now = [
            container("a", "exited", "Exited (1) 1 second ago"),
            container("b", "exited", "Exited (0) 1 second ago"),
            container("c", "exited", "Exited (143) 1 second ago"),
            container("d", "exited", "Exited (1) 1 hour ago"),
        ];
        let expected = HashSet::from(["c".to_owned()]);
        assert_eq!(
            unexpected_exits(&before, &now, &expected),
            [("a-name".to_owned(), 1)]
        );
    }
}
