use crate::McpError;
use gateway_types::GatewayMcpTool;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug)]
pub struct SchemaLimits {
    pub maximum_bytes: usize,
    pub maximum_depth: usize,
    pub maximum_properties: usize,
    pub maximum_string_bytes: usize,
}
impl Default for SchemaLimits {
    fn default() -> Self {
        Self {
            maximum_bytes: 262_144,
            maximum_depth: 32,
            maximum_properties: 1_024,
            maximum_string_bytes: 65_536,
        }
    }
}

pub fn validate_tools(
    tools: &[GatewayMcpTool],
    limits: SchemaLimits,
) -> Result<Vec<(GatewayMcpTool, String)>, McpError> {
    if tools.len() > 10_000 {
        return Err(McpError::TooLarge);
    }
    tools
        .iter()
        .map(|tool| {
            if tool.tool_name.is_empty() || tool.tool_name.len() > 255 {
                return Err(McpError::Invalid);
            }
            if tool
                .description
                .as_deref()
                .is_some_and(|value| value.len() > limits.maximum_string_bytes)
            {
                return Err(McpError::TooLarge);
            }
            let encoded = serde_json::to_vec(&tool.input_schema).map_err(|_| McpError::Invalid)?;
            if encoded.len() > limits.maximum_bytes {
                return Err(McpError::TooLarge);
            }
            let mut properties = 0;
            validate_value(&tool.input_schema, 0, &mut properties, limits)?;
            if !tool.input_schema.is_object()
                || tool
                    .input_schema
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| value != "object")
            {
                return Err(McpError::Invalid);
            }
            Ok((tool.clone(), format!("{:x}", Sha256::digest(encoded))))
        })
        .collect()
}

fn validate_value(
    value: &serde_json::Value,
    depth: usize,
    properties: &mut usize,
    limits: SchemaLimits,
) -> Result<(), McpError> {
    if depth > limits.maximum_depth {
        return Err(McpError::TooLarge);
    }
    match value {
        serde_json::Value::String(value) if value.len() > limits.maximum_string_bytes => {
            Err(McpError::TooLarge)
        }
        serde_json::Value::Array(items) => {
            for item in items {
                validate_value(item, depth + 1, properties, limits)?;
            }
            Ok(())
        }
        serde_json::Value::Object(items) => {
            *properties = properties.saturating_add(items.len());
            if *properties > limits.maximum_properties {
                return Err(McpError::TooLarge);
            }
            if let Some(kind) = items.get("type") {
                let valid = kind.as_str().is_some_and(valid_type)
                    || kind.as_array().is_some_and(|values| {
                        !values.is_empty()
                            && values
                                .iter()
                                .all(|value| value.as_str().is_some_and(valid_type))
                    });
                if !valid {
                    return Err(McpError::Invalid);
                }
            }
            if items
                .get("properties")
                .is_some_and(|value| !value.is_object())
            {
                return Err(McpError::Invalid);
            }
            if let Some(required) = items.get("required") {
                let Some(required) = required.as_array() else {
                    return Err(McpError::Invalid);
                };
                if required.iter().any(|value| value.as_str().is_none()) {
                    return Err(McpError::Invalid);
                }
            }
            if items
                .get("$ref")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| !value.starts_with("#/"))
            {
                return Err(McpError::Invalid);
            }
            for value in items.values() {
                validate_value(value, depth + 1, properties, limits)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn valid_type(value: &str) -> bool {
    matches!(
        value,
        "null" | "boolean" | "object" | "array" | "number" | "integer" | "string"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_types::{McpServerId, ToolAnnotations};
    fn tool(schema: serde_json::Value) -> GatewayMcpTool {
        GatewayMcpTool {
            server_id: McpServerId::new(),
            tool_name: "read".into(),
            description: None,
            input_schema: schema,
            annotations: ToolAnnotations::default(),
        }
    }
    #[test]
    fn validates_and_hashes_json_schema() {
        let values = validate_tools(
            &[tool(
                serde_json::json!({"type":"object","properties":{"id":{"type":"integer"}}}),
            )],
            SchemaLimits::default(),
        )
        .unwrap();
        assert_eq!(values[0].1.len(), 64)
    }
    #[test]
    fn rejects_invalid_or_deep_schema() {
        assert_eq!(
            validate_tools(
                &[tool(serde_json::json!({"type":"wat"}))],
                SchemaLimits::default()
            )
            .unwrap_err(),
            McpError::Invalid
        );
        let limits = SchemaLimits {
            maximum_depth: 1,
            ..SchemaLimits::default()
        };
        assert_eq!(
            validate_tools(
                &[tool(
                    serde_json::json!({"type":"object","properties":{"id":{"type":"integer"}}})
                )],
                limits
            )
            .unwrap_err(),
            McpError::TooLarge
        )
    }
}
