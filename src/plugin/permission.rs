use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    ReadBuffer,
    ProposeEdit,
    DrawPane,
    RegisterCommand,
    ListenEvents,
    BindKeys,
}
