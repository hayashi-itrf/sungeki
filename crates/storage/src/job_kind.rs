use rusqlite::ToSql;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, ValueRef};

/// Job IDs are globally unique across kinds. Kind is immutable after creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Review,
    Speech,
    Render,
    Loudness,
    FramePreview,
    AudioPreview,
    BuildVideo,
    Collection,
    Proxy,
}
impl ToSql for JobKind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match self {
            Self::Review => "review",
            Self::Speech => "speech",
            Self::Render => "render",
            Self::Loudness => "loudness",
            Self::FramePreview => "frame_preview",
            Self::AudioPreview => "audio_preview",
            Self::BuildVideo => "build_video",
            Self::Collection => "collection",
            Self::Proxy => "proxy",
        }
        .into())
    }
}
impl FromSql for JobKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "review" => Ok(Self::Review),
            "speech" => Ok(Self::Speech),
            "render" => Ok(Self::Render),
            "loudness" => Ok(Self::Loudness),
            "frame_preview" => Ok(Self::FramePreview),
            "audio_preview" => Ok(Self::AudioPreview),
            "build_video" => Ok(Self::BuildVideo),
            "collection" => Ok(Self::Collection),
            "proxy" => Ok(Self::Proxy),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}
