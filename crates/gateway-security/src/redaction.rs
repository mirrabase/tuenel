use gateway_types::{InspectionContent, SanitizedEvidence};
use serde_json::Value;

pub fn redact(content: &InspectionContent, evidence: &[SanitizedEvidence]) -> InspectionContent {
    match content {
        InspectionContent::PromptText(value) => {
            InspectionContent::PromptText(redact_text(value, evidence))
        }
        InspectionContent::ModelOutput(value) => {
            InspectionContent::ModelOutput(redact_text(value, evidence))
        }
        InspectionContent::StructuredInput(value) => {
            InspectionContent::StructuredInput(redact_json(value, evidence))
        }
        InspectionContent::ToolArguments(value) => {
            InspectionContent::ToolArguments(redact_json(value, evidence))
        }
        InspectionContent::ToolResult(value) => {
            InspectionContent::ToolResult(redact_json(value, evidence))
        }
    }
}

pub fn redact_text(value: &str, evidence: &[SanitizedEvidence]) -> String {
    let mut spans = evidence
        .iter()
        .filter_map(|item| Some((item.start?, item.end?)))
        .filter(|(start, end)| start < end && *end <= value.len())
        .collect::<Vec<_>>();
    spans.sort_unstable();
    let spans = spans
        .into_iter()
        .fold(Vec::<(usize, usize)>::new(), |mut merged, (start, end)| {
            if let Some(last) = merged.last_mut().filter(|last| start <= last.1) {
                last.1 = last.1.max(end)
            } else {
                merged.push((start, end))
            }
            merged
        });
    let mut output = value.to_owned();
    for (start, end) in spans.into_iter().rev() {
        output.replace_range(start..end, "[REDACTED]");
    }
    output
}

fn redact_json(value: &Value, evidence: &[SanitizedEvidence]) -> Value {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    serde_json::from_str(&redact_text(&serialized, evidence))
        .unwrap_or_else(|_| fully_redact_strings(value))
}

fn fully_redact_strings(value: &Value) -> Value {
    match value {
        Value::String(_) => Value::String("[REDACTED]".into()),
        Value::Array(items) => Value::Array(items.iter().map(fully_redact_strings).collect()),
        Value::Object(items) => Value::Object(
            items
                .iter()
                .map(|(key, value)| (key.clone(), fully_redact_strings(value)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redacts_text_and_structured_values() {
        let evidence = SanitizedEvidence {
            redacted: "safe...[REDACTED]".into(),
            sha256: "hash".into(),
            start: Some(10),
            end: Some(27),
        };
        let content =
            InspectionContent::ToolArguments(serde_json::json!({"email":"alice@example.com"}));
        let InspectionContent::ToolArguments(value) = redact(&content, &[evidence]) else {
            panic!()
        };
        assert!(!value.to_string().contains("alice@example.com"));
        assert!(value.to_string().contains("REDACTED"));
    }
}
