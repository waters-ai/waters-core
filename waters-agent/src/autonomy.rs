use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutonomyLevel {
    L0 = 0,
    L1 = 1,
    L2 = 2,
    L3 = 3,
    L4 = 4,
}

impl fmt::Display for AutonomyLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AutonomyLevel::L0 => write!(f, "L0"),
            AutonomyLevel::L1 => write!(f, "L1"),
            AutonomyLevel::L2 => write!(f, "L2"),
            AutonomyLevel::L3 => write!(f, "L3"),
            AutonomyLevel::L4 => write!(f, "L4"),
        }
    }
}

pub struct AutonomyEngine {
    current_level: AutonomyLevel,
}

impl AutonomyEngine {
    pub fn new() -> Self {
        AutonomyEngine {
            current_level: AutonomyLevel::L0,
        }
    }

    pub fn determine_level(
        &mut self,
        kafka_ok: bool,
        llm_ok: bool,
        llm_is_local: bool,
    ) -> AutonomyLevel {
        let new_level = if kafka_ok && llm_ok && !llm_is_local {
            AutonomyLevel::L0
        } else if kafka_ok && llm_ok && llm_is_local {
            AutonomyLevel::L1
        } else if !kafka_ok && llm_ok && llm_is_local {
            AutonomyLevel::L2
        } else if !kafka_ok && llm_ok {
            AutonomyLevel::L3
        } else {
            AutonomyLevel::L4
        };

        if new_level != self.current_level {
            info!(
                "Autonomy level changed: {} -> {} (kafka={}, llm={}, local={})",
                self.current_level, new_level, kafka_ok, llm_ok, llm_is_local
            );
            self.current_level = new_level;
        }
        new_level
    }

    pub fn current(&self) -> AutonomyLevel {
        self.current_level
    }

    pub fn can_publish(&self) -> bool {
        matches!(self.current_level, AutonomyLevel::L0 | AutonomyLevel::L1)
    }

    pub fn can_process(&self) -> bool {
        matches!(
            self.current_level,
            AutonomyLevel::L0 | AutonomyLevel::L1 | AutonomyLevel::L2 | AutonomyLevel::L3
        )
    }

    pub fn buffer_instead(&self) -> bool {
        matches!(self.current_level, AutonomyLevel::L2 | AutonomyLevel::L3)
    }
}
