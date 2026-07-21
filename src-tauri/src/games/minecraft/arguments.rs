use serde_json::Value;
use std::collections::HashMap;

use crate::games::minecraft::classpath::os_matches;

pub fn substitute(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (k, v) in vars {
        result = result.replace(&format!("${{{}}}", k), v);
    }
    result
}

pub fn extract_argument_list(value: &Value, vars: &HashMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    let arr = match value.as_array() {
        Some(a) => a,
        None => return out,
    };

    for item in arr {
        match item {
            Value::String(s) => out.push(substitute(s, vars)),
            Value::Object(_) => {
                let rules_ok = item["rules"]
                    .as_array()
                    .map(|rules| {
                        let mut allowed = false;
                        for rule in rules {
                            let action_allow = rule["action"].as_str() == Some("allow");
                            let matches_os = rule.get("os").map(os_matches).unwrap_or(true);
                            if matches_os && rule.get("features").is_none() {
                                allowed = action_allow;
                            }
                        }
                        allowed
                    })
                    .unwrap_or(false);

                if rules_ok {
                    if let Some(val) = item["value"].as_str() {
                        out.push(substitute(val, vars));
                    } else if let Some(vals) = item["value"].as_array() {
                        for v in vals {
                            if let Some(s) = v.as_str() {
                                out.push(substitute(s, vars));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}
