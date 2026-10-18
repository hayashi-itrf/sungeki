use super::VoicevoxError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Catalog,
    Analysis,
    Synthesis,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    Configuration(String),
    Input(VoicevoxError),
    Cancelled,
    Metadata(String),
    Http { stage: Stage, status: u16 },
    Timeout(Stage),
    Connection(Stage),
    TooLarge { stage: Stage, limit: usize },
}
impl TransportError {
    /// A broken/aborted HTTP exchange does not prove the engine stopped work.
    /// The owner must settle the engine before reusing its execution lease.
    pub fn requires_engine_settlement(&self) -> bool {
        matches!(
            self,
            Self::Timeout(stage) | Self::Connection(stage) | Self::TooLarge { stage, .. } | Self::Http { stage, .. }
                if *stage != Stage::Catalog
        )
    }
}
impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configuration(message) | Self::Metadata(message) => f.write_str(message),
            Self::Input(error) => error.fmt(f),
            Self::Cancelled => f.write_str("音声エンジンの処理をキャンセルしました"),
            Self::Http { stage, status } => write!(f, "音声エンジン {stage:?}: HTTP {status}"),
            Self::Timeout(stage) => write!(f, "音声エンジン {stage:?}: 待機時間を超過しました"),
            Self::Connection(stage) => write!(f, "音声エンジン {stage:?}: 通信に失敗しました"),
            Self::TooLarge { stage, limit } => {
                write!(
                    f,
                    "音声エンジン {stage:?}: 応答が{limit}バイトを超えています"
                )
            }
        }
    }
}
impl std::error::Error for TransportError {}
