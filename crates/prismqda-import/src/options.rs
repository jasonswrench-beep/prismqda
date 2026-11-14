use prismqda_engine::{SourceType, UnitKind};

/// Which of a photograph's own facts to keep as variables of a case made for that picture. All three are off unless asked for: a date
/// and a camera describe the picture, but a precise place can lead to a person's home.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
pub struct PictureVariables {
    /// When it was taken: the variables Date taken, Month taken, and Year taken.
    #[serde(default)]
    pub when: bool,
    /// What it was taken with: the variable Camera.
    #[serde(default)]
    pub camera: bool,
    /// Where it was taken: the variables Latitude and Longitude.
    #[serde(default)]
    pub place: bool,
}

impl PictureVariables {
    pub fn any(&self) -> bool {
        self.when || self.camera || self.place
    }
}

/// Choices that apply to any import. Format-specific choices (spreadsheet columns, social media
/// layouts) have their own settings types.
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Name for the source. Defaults to the file name without its extension.
    pub name: Option<String>,
    /// The folder the source is filed under in the project.
    pub folder: String,
    /// Overrides the source type the format would give.
    pub source_type: Option<SourceType>,
    /// Overrides the default unit for the source type.
    pub unit_kind: Option<UnitKind>,
    /// Transcripts: speaker names whose turns are left out of coding, for example "Interviewer".
    pub interviewer_labels: Vec<String>,
    /// SHA-256 of the original file, lowercase hex. Set by the caller that read the file.
    pub file_hash: Option<String>,
    /// Pictures: which of the camera's facts become variables of a case made for the picture.
    pub picture_variables: PictureVariables,
}
