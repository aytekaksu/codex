//! Tool specs for the plaintext `external_agents` namespace.

use crate::tools::handlers::multi_agents_spec::SpawnAgentToolOptions;
use crate::tools::handlers::multi_agents_v2::external_agents::PLAINTEXT_MESSAGE_MAX_BYTES;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub(crate) const EXTERNAL_AGENTS_NAMESPACE_DESCRIPTION: &str =
    "Plaintext tools for spawning and messaging allowlisted non-OpenAI workers such as Muse Spark.";

pub(crate) fn create_external_spawn_agent_tool(options: SpawnAgentToolOptions) -> ToolSpec {
    let mut properties = BTreeMap::from([
        (
            "message".to_string(),
            JsonSchema::string(Some(format!(
                "Full plaintext task for the worker. This is the only assignment the child receives. Cap {PLAINTEXT_MESSAGE_MAX_BYTES} bytes. Do not send encrypted collaboration payloads."
            ))),
        ),
        (
            "task_name".to_string(),
            JsonSchema::string(Some(
                "Task name for the new agent. Muse Spark workers must start with muse_."
                    .to_string(),
            )),
        ),
        (
            "agent_type".to_string(),
            JsonSchema::string(Some(format!(
                "Allowlisted non-OpenAI role. Omit when task_name starts with muse_, which selects muse_spark.\n{}",
                options.agent_type_description
            ))),
        ),
    ]);
    if !options.expose_agent_type {
        properties.remove("agent_type");
    }
    if options.expose_spawn_agent_model_overrides {
        properties.insert(
            "model".to_string(),
            JsonSchema::string(Some(
                "Optional model override. Do not pass muse-spark-1.3 from a ChatGPT parent."
                    .to_string(),
            )),
        );
        properties.insert(
            "reasoning_effort".to_string(),
            JsonSchema::string(Some(
                "Optional reasoning effort override for the new agent.".to_string(),
            )),
        );
    }

    ToolSpec::Function(ResponsesApiTool {
        name: "spawn_agent".to_string(),
        description: external_spawn_agent_description(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["task_name".to_string(), "message".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

pub(crate) fn create_external_send_message_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: "send_message".to_string(),
        description: "Send a plaintext message to an allowlisted non-OpenAI worker such as Muse Spark. The message is delivered immediately as user input. Do not use collaboration.send_message for Muse.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: plaintext_target_message_parameters(),
        output_schema: None,
    })
}

pub(crate) fn create_external_followup_task_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: "followup_task".to_string(),
        description: "Send a plaintext follow-up task to an existing non-root allowlisted non-OpenAI worker and trigger a turn if it is idle. Do not use collaboration.followup_task for Muse.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: plaintext_target_message_parameters(),
        output_schema: None,
    })
}

fn plaintext_target_message_parameters() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            (
                "target".to_string(),
                JsonSchema::string(Some(
                    "Relative or canonical task name of an allowlisted non-OpenAI worker."
                        .to_string(),
                )),
            ),
            (
                "message".to_string(),
                JsonSchema::string(Some(format!(
                    "Plaintext user input for the worker. Cap {PLAINTEXT_MESSAGE_MAX_BYTES} bytes."
                ))),
            ),
        ]),
        Some(vec!["target".to_string(), "message".to_string()]),
        Some(false.into()),
    )
}

fn external_spawn_agent_description() -> String {
    format!(
        r#"Spawn an allowlisted non-OpenAI worker. Muse Spark is selected when task_name starts with muse_ (for example muse_inventory). Put the complete assignment in plaintext `message`; the child does not inherit parent history and cannot read encrypted collaboration payloads. After you spawn, call one long collaboration.wait_agent, then one collaboration.list_agents. Inspect files and tests yourself; do not treat child prose as verification. Cap `message` at {PLAINTEXT_MESSAGE_MAX_BYTES} bytes. Do not use collaboration.spawn_agent for Muse."#
    )
}
