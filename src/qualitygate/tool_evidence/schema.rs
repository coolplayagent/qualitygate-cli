//! Exported closed JSON protocol for tool observations; binding is verified at run time.

use serde_json::{Value, json};

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","additionalProperties":false,"properties":properties,"required":required})
}

pub fn document() -> Value {
    let text = json!({"type":"string","minLength":1,"maxLength":4096});
    let count = json!({"type":"integer","minimum":0,"maximum":9_223_372_036_854_775_807_u64});
    let decimal = object(
        json!({
            "coefficient":{"type":"string","pattern":"^-?(0|[1-9][0-9]{0,37})$","maxLength":39},
            "scale":{"type":"integer","minimum":0,"maximum":18}
        }),
        &["coefficient", "scale"],
    );
    let location = object(
        json!({
            "file":text,"start_line":{"type":"integer","minimum":1},
            "end_line":{"type":"integer","minimum":1},"symbol":text
        }),
        &["file", "start_line", "end_line", "symbol"],
    );
    let definition = object(
        json!({"metric":text,"unit":text,"measurement":text}),
        &["metric", "unit", "measurement"],
    );
    let inventory_entry = object(
        json!({"namespace":text,"name":text,"location":{"$ref":"#/$defs/location"}}),
        &["namespace", "name", "location"],
    );
    let inventory = object(
        json!({"files":{"type":"array","maxItems":50000,"items":text},"entities":{"type":"array","maxItems":50000,"items":{"$ref":"#/$defs/inventory_entry"}}}),
        &["files", "entities"],
    );
    let scope = object(
        json!({"project":text,"target":{"type":["string","null"],"minLength":1,"maxLength":4096},"configuration":{"type":["string","null"],"minLength":1,"maxLength":4096}}),
        &["project"],
    );
    let metric = object(
        json!({
            "kind":{"const":"metric"},"id":text,"location":{"$ref":"#/$defs/location"},
            "metric":text,"value":{"$ref":"#/$defs/decimal"},"unit":text,"measurement":text
        }),
        &[
            "kind",
            "id",
            "location",
            "metric",
            "value",
            "unit",
            "measurement",
        ],
    );
    let pair = object(
        json!({
            "kind":{"const":"clone_pair"},"id":text,"left":{"$ref":"#/$defs/location"},"right":{"$ref":"#/$defs/location"},
            "similarity":{"$ref":"#/$defs/decimal"},"unit":{"const":"ratio"},"measurement":text,
            "left_nodes":count,"right_nodes":count
        }),
        &[
            "kind",
            "id",
            "left",
            "right",
            "similarity",
            "unit",
            "measurement",
            "left_nodes",
            "right_nodes",
        ],
    );
    let mut schema = object(
        json!({
            "protocol_version":{"const":1},"schema":{"enum":["function_metric","clone_pair"]},"schema_version":{"const":1},
            "producer":text,"snapshot_digest":{"type":"string","pattern":"^sha256:[0-9a-f]{64}$"},
            "tool_id":text,"tool_version":text,"artifact_digest":{"type":"string","maxLength":128},
            "scope":{"$ref":"#/$defs/scope"},"inventory":{"$ref":"#/$defs/inventory"},"complete":{"type":"boolean"},
            "minimum_lines":{"type":"integer","minimum":1,"maximum":4_294_967_295_u64},
            "minimum_nodes":{"type":"integer","minimum":0,"maximum":4_294_967_295_u64},
            "similarity_floor":{"$ref":"#/$defs/decimal"},
            "metric_definitions":{"type":"array","maxItems":64,"items":{"$ref":"#/$defs/metric_definition"}},
            "clone_measurement":{"type":["string","null"],"minLength":1,"maxLength":4096},
            "records":{"type":"array","maxItems":50000,"items":{"oneOf":[{"$ref":"#/$defs/metric"},{"$ref":"#/$defs/clone_pair"}]}}
        }),
        &[
            "protocol_version",
            "schema",
            "schema_version",
            "producer",
            "snapshot_digest",
            "tool_id",
            "tool_version",
            "artifact_digest",
            "scope",
            "inventory",
            "complete",
            "records",
        ],
    );
    schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    schema["title"] = json!("Qualitygate tool facts v1");
    schema["$defs"] = json!({"decimal":decimal,"location":location,"scope":scope,"inventory":inventory,"inventory_entry":inventory_entry,"metric_definition":definition,"metric":metric,"clone_pair":pair});
    schema["description"] = json!(
        "Structural protocol only. Exact decimals, inventory, measurement units, execution identity, scope and completeness require semantic validation; producer claims are not authentication."
    );
    schema
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
