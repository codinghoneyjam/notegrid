// <META - FILE SUMMARY - Project model, validation (I-xx), canonical serialisation, GM tables, migrate scaffold (R01).>

pub mod canonical;
pub mod gm;
pub mod migrate;
pub mod model;
pub mod validate;

pub use canonical::to_canonical_json;
pub use gm::gm_tables;
pub use migrate::migrate;
pub use model::{Lane, LaneKind, LoopRegion, MeterItem, Note, Project, TempoItem};
pub use validate::{validate, validate_bytes, Issue, Report};

/// C2 trait surface (ng-score).
impl Project {
    pub fn validate(&self) -> Report {
        validate(self)
    }
    pub fn to_canonical_json(&self) -> String {
        to_canonical_json(self)
    }
}

pub trait ScoreExporter {
    type Options: Default;
    fn id(&self) -> &'static str;
    fn extension(&self) -> &'static str;
    fn mime(&self) -> &'static str;
    fn export(&self, project: &Project, opts: &Self::Options) -> Result<ExportOutput, ExportError>;
}

pub struct ExportOutput {
    pub bytes: Vec<u8>,
    pub warnings: Vec<Issue>,
}

pub trait ScoreImporter {
    type Options: Default;
    fn import(&self, bytes: &[u8], opts: &Self::Options) -> Result<ImportOutput, ImportError>;
}

pub struct ImportOutput {
    pub project: Project,
    pub warnings: Vec<Issue>,
}

#[derive(Debug, Clone)]
pub struct ExportError {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ImportError {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ScoreError {
    pub code: &'static str,
    pub message: String,
}
