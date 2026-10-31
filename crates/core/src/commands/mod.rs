use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandType {
    Tool,
    Skill,
    Slash,
    /// Extension-registered slash command (e.g. a third-party extension's `/review`).
    Extension,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub command_type: CommandType,
}
