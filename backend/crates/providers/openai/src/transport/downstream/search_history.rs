//! 为回放搜索历史的 OAuth 请求补齐缓存搜索声明，保留历史与调用方工具选择

use serde_json::{Map, Value, json};

fn declares_search(tools: Option<&Value>) -> bool {
    tools.and_then(Value::as_array).is_some_and(|tools| {
        tools.iter().any(|tool| {
            tool.get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.trim().starts_with("web_search"))
        })
    })
}

pub(super) fn declare_search_history_tool(body: &mut Map<String, Value>, lite: bool) {
    let Some(input) = body.get("input").and_then(Value::as_array) else {
        return;
    };
    if !input
        .iter()
        .any(|item| item.get("type").and_then(Value::as_str) == Some("web_search_call"))
        || declares_search(body.get("tools"))
        || input.iter().any(|item| {
            item.get("type").and_then(Value::as_str) == Some("additional_tools")
                && declares_search(item.get("tools"))
        })
    {
        return;
    }
    // 非法显式工具结构仍交给上游校验，不用兼容补齐掩盖请求错误
    if body
        .get("tools")
        .is_some_and(|tools| !tools.is_null() && !tools.is_array())
    {
        return;
    }
    let caller_has_tools = body
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| !tools.is_empty())
        || input.iter().any(|item| {
            item.get("type").and_then(Value::as_str) == Some("additional_tools")
                && item
                    .get("tools")
                    .and_then(Value::as_array)
                    .is_some_and(|tools| !tools.is_empty())
        });
    let additional = input
        .iter()
        .position(|item| item.get("type").and_then(Value::as_str) == Some("additional_tools"));
    let insert_at = input.len()
        - usize::from(input.last().is_some_and(|item| {
            item.get("type").and_then(Value::as_str) == Some("compaction_trigger")
        }));
    let tool = json!({"type":"web_search", "external_web_access":false});
    if lite {
        let Some(input) = body.get_mut("input").and_then(Value::as_array_mut) else {
            return;
        };
        if let Some(index) = additional {
            let Some(item) = input[index].as_object_mut() else {
                return;
            };
            let tools = item.entry("tools").or_insert_with(|| json!([]));
            if tools.is_null() {
                *tools = json!([]);
            }
            let Some(tools) = tools.as_array_mut() else {
                return;
            };
            tools.push(tool);
        } else {
            input.insert(
                insert_at,
                json!({"type":"additional_tools", "role":"developer", "tools":[tool]}),
            );
        }
    } else {
        let tools = body.entry("tools").or_insert_with(|| json!([]));
        if tools.is_null() {
            *tools = json!([]);
        }
        let Some(tools) = tools.as_array_mut() else {
            return;
        };
        tools.push(tool);
    }
    // 只补足历史解释所需声明；无工具请求不会因此获得新的工具执行权限
    if !caller_has_tools
        && body.get("tool_choice").is_none_or(|choice| {
            choice.is_null()
                || choice.as_str().is_some_and(|choice| {
                    matches!(
                        choice.trim().to_ascii_lowercase().as_str(),
                        "" | "auto" | "none"
                    )
                })
        })
    {
        body.insert("tool_choice".to_owned(), json!("none"));
    }
}
