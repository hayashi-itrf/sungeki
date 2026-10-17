/// Internal diagnostic data; the host maps it to the public API error details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationDiagnostic {
    TrackOverlap {
        track_id: String,
        track_name: String,
        first_clip_id: String,
        second_clip_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError(pub String, pub Option<ValidationDiagnostic>);

impl ValidationError {
    pub fn new(message: String) -> Self {
        Self(message, None)
    }
    pub fn with_diagnostic(mut self, diagnostic: ValidationDiagnostic) -> Self {
        self.1 = Some(diagnostic);
        self
    }
}
impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for ValidationError {}
