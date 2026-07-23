#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McpCapabilities {
    pub tools: bool,
    pub tool_list_changed: bool,
}
