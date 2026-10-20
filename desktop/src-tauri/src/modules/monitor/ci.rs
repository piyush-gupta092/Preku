//! GitHub Actions watcher (PRD M5): latest workflow run per repository.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::foundation::errors::{PrekuError, Result};

/// Keychain key for the GitHub personal access token.
pub const TOKEN_KEY: &str = "github.token";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CiRun {
    #[ts(type = "number")]
    pub id: u64,
    pub name: String,
    pub branch: String,
    /// `queued`, `in_progress`, `completed`, …
    pub status: String,
    /// When completed: `success`, `failure`, `cancelled`, …
    pub conclusion: Option<String>,
    pub url: String,
}

impl CiRun {
    pub fn is_running(&self) -> bool {
        self.status != "completed"
    }
}

/// What the UI shows per watched repository.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CiStatus {
    pub repo: String,
    pub run: Option<CiRun>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CiEvent {
    Started(CiRun),
    Succeeded(CiRun),
    Failed(CiRun),
}

/// Compares the previous and current latest run. Nothing is reported for the very first
/// observation, so starting Preku doesn't replay old results.
pub fn transition(previous: Option<&CiRun>, current: &CiRun) -> Option<CiEvent> {
    let previous = previous?;
    let same_run = previous.id == current.id;
    if current.is_running() {
        return (!same_run || !previous.is_running()).then(|| CiEvent::Started(current.clone()));
    }
    let newly_finished = !same_run || previous.is_running();
    if !newly_finished {
        return None;
    }
    Some(match current.conclusion.as_deref() {
        Some("success") => CiEvent::Succeeded(current.clone()),
        Some("failure" | "timed_out" | "startup_failure") => CiEvent::Failed(current.clone()),
        // cancelled / skipped / neutral: not worth an alert.
        _ => return None,
    })
}

#[derive(Deserialize)]
struct RunsResponse {
    workflow_runs: Vec<ApiRun>,
}

#[derive(Deserialize)]
struct ApiRun {
    id: u64,
    name: Option<String>,
    head_branch: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
    html_url: String,
}

pub async fn latest_run(
    client: &reqwest::Client,
    repo: &str,
    token: Option<&str>,
) -> Result<Option<CiRun>> {
    let url = format!("https://api.github.com/repos/{repo}/actions/runs?per_page=1");
    let mut request = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(http_err)?;
    let status = response.status();
    if !status.is_success() {
        // Message for the user; the token never appears in it.
        return Err(PrekuError::InvalidInput(match status.as_u16() {
            401 => "GitHub rejected the token".into(),
            403 | 429 => "GitHub rate limit reached; add a token".into(),
            404 => "Repository not found (private? add a token)".into(),
            code => format!("GitHub returned {code}"),
        }));
    }
    let body: RunsResponse = response.json().await.map_err(http_err)?;
    Ok(body.workflow_runs.into_iter().next().map(|run| CiRun {
        id: run.id,
        name: run.name.unwrap_or_default(),
        branch: run.head_branch.unwrap_or_default(),
        status: run.status.unwrap_or_default(),
        conclusion: run.conclusion,
        url: run.html_url,
    }))
}

pub fn http_err(err: reqwest::Error) -> PrekuError {
    // `without_url` keeps query strings out of messages and logs.
    PrekuError::Io(std::io::Error::other(err.without_url()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(id: u64, status: &str, conclusion: Option<&str>) -> CiRun {
        CiRun {
            id,
            name: "CI".into(),
            branch: "main".into(),
            status: status.into(),
            conclusion: conclusion.map(Into::into),
            url: String::new(),
        }
    }

    #[test]
    fn first_observation_is_silent() {
        assert_eq!(
            transition(None, &run(1, "completed", Some("failure"))),
            None
        );
    }

    #[test]
    fn reports_start_and_result_once() {
        let queued = run(2, "in_progress", None);
        let old = run(1, "completed", Some("success"));
        assert!(matches!(
            transition(Some(&old), &queued),
            Some(CiEvent::Started(_))
        ));
        assert_eq!(transition(Some(&queued), &queued), None);

        let failed = run(2, "completed", Some("failure"));
        assert!(matches!(
            transition(Some(&queued), &failed),
            Some(CiEvent::Failed(_))
        ));
        assert_eq!(transition(Some(&failed), &failed), None);
    }

    #[test]
    fn a_run_finishing_between_polls_still_reports() {
        let old = run(1, "completed", Some("failure"));
        let new = run(2, "completed", Some("success"));
        assert!(matches!(
            transition(Some(&old), &new),
            Some(CiEvent::Succeeded(_))
        ));
    }

    #[test]
    fn cancelled_runs_are_quiet() {
        let running = run(3, "in_progress", None);
        assert_eq!(
            transition(Some(&running), &run(3, "completed", Some("cancelled"))),
            None
        );
    }
}
