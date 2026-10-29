//! Stage 1: tool discovery and explicit --version inspection only.
//! No connection, cluster, credential store, service control or Manager dependency.
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex};

mod discovery;
#[cfg(test)]
mod tests;
mod version;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    Busy,
    ConfirmationRequired,
    StaleCandidate,
    Unavailable,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}
impl Error {
    fn new(code: ErrorCode, message: &str) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    FairwayRuntime,
    Environment,
    Homebrew,
    System,
    PostgresApp,
    Path,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    Postgres,
    Initdb,
    PgCtl,
    Psql,
    PgIsready,
    PgDump,
    PgRestore,
    PgDumpall,
}
impl Tool {
    const ALL: [Self; 8] = [
        Self::Postgres,
        Self::Initdb,
        Self::PgCtl,
        Self::Psql,
        Self::PgIsready,
        Self::PgDump,
        Self::PgRestore,
        Self::PgDumpall,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Initdb => "initdb",
            Self::PgCtl => "pg_ctl",
            Self::Psql => "psql",
            Self::PgIsready => "pg_isready",
            Self::PgDump => "pg_dump",
            Self::PgRestore => "pg_restore",
            Self::PgDumpall => "pg_dumpall",
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: Option<u16>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ToolIssue {
    Missing,
    UnsafeFile,
    ProbeFailed,
    InvalidVersion,
    DeadlineExceeded,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolView {
    pub name: Tool,
    pub path: Option<String>,
    pub version: Option<Version>,
    pub issue: Option<ToolIssue>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ToolsetCheck {
    NotChecked,
    Missing,
    Invalid,
    MixedVersions,
    Verified,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub source: Source,
    pub bin_directory: String,
    pub tools: Vec<ToolView>,
    /// Verified means matching --version output, never cluster qualification/ownership.
    pub server_tools: ToolsetCheck,
    pub client_tools: ToolsetCheck,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub candidates: Vec<Candidate>,
    pub truncated: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectRequest {
    pub candidate_id: String,
    /// Consent is only for fixed --version invocations, never service startup.
    pub confirmed: bool,
}

/// Construction performs no IO. Calls are independent from PHP/MySQL startup.
pub struct Discovery {
    runtime: Option<PathBuf>,
    gate: Mutex<()>,
}
impl Discovery {
    pub fn new(runtime: Option<PathBuf>) -> Self {
        Self {
            runtime,
            gate: Mutex::new(()),
        }
    }
    pub fn discover(&self) -> Inventory {
        discovery::inventory(&discovery::roots(self.runtime.as_deref()))
    }
    pub fn inspect(&self, request: InspectRequest) -> Result<Candidate> {
        if !request.confirmed {
            return Err(Error::new(
                ErrorCode::ConfirmationRequired,
                "Confirm read-only PostgreSQL tool version inspection.",
            ));
        }
        let _guard = self.gate.try_lock().map_err(|_| {
            Error::new(
                ErrorCode::Busy,
                "Another PostgreSQL version inspection is running.",
            )
        })?;
        discovery::inspect(self.discover(), &request.candidate_id, version::probe)
    }
}
