use tokio::time::Instant;

use serde::Serialize;

use super::tube::{BuriedPos, ReadyPos};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JobState {
    Ready { pos: ReadyPos },
    Delayed { until: Instant },
    Reserved { deadline: Instant },
    Buried { pos: BuriedPos },
}

// This impl is used to allow JobStats to be serialised to YAML.
impl Serialize for JobState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use JobState::*;

        serializer.serialize_str(match self {
            Ready { .. } => "ready",
            Delayed { .. } => "delayed",
            Reserved { .. } => "reserved",
            Buried { .. } => "buried",
        })
    }
}
