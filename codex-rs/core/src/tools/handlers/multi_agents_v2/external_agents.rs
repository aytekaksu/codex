//! Plaintext `external_agents` helpers for Muse and other non-OpenAI workers.

use crate::agent::AgentMetadata;
use crate::agent::role::resolve_role_config;
use crate::config::Config;
use crate::function_tool::FunctionCallError;
use crate::tools::handlers::multi_agents_common::infer_muse_spark_role;
use crate::tools::handlers::multi_agents_common::looks_like_muse_spark_name;
use crate::tools::handlers::multi_agents_v2::message_tool::message_content;
use codex_protocol::user_input::UserInput;
use codex_tools::ToolName;

pub(crate) const EXTERNAL_AGENTS_NAMESPACE: &str = "external_agents";
pub(crate) const PLAINTEXT_MESSAGE_MAX_BYTES: usize = 8 * 1024;
pub(crate) const OMITTED_ENCRYPTED_PAYLOAD_STUB: &str = "[encrypted inter-agent payload omitted; use the parent conversation and any plaintext headers above as the task]";

const COLLABORATION_MUSE_ERROR: &str = "Muse Spark workers cannot use encrypted collaboration. Call external_agents.spawn_agent, external_agents.send_message, or external_agents.followup_task with a plaintext `message` that contains the full task.";

/// Which multi-agent transport a v2 handler is advertising.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AgentTransport {
    #[default]
    Collaboration,
    ExternalAgents,
}

pub(crate) fn is_external_agents_namespace(tool_name: &ToolName) -> bool {
    tool_name.namespace.as_deref() == Some(EXTERNAL_AGENTS_NAMESPACE)
}

pub(crate) fn is_external_agents_mutating_tool(tool_name: &ToolName) -> bool {
    is_external_agents_namespace(tool_name)
        && matches!(
            tool_name.name.as_str(),
            "spawn_agent" | "send_message" | "followup_task"
        )
}

pub(crate) fn collaboration_muse_error() -> FunctionCallError {
    FunctionCallError::RespondToModel(COLLABORATION_MUSE_ERROR.to_string())
}

pub(crate) fn reject_encrypted_external_agents_args(
    encrypted_function_args: &Option<Vec<String>>,
) -> Result<(), FunctionCallError> {
    if encrypted_function_args
        .as_ref()
        .is_some_and(|fields| !fields.is_empty())
    {
        return Err(FunctionCallError::RespondToModel(
            "external_agents only accepts plaintext messages; do not encrypt the payload"
                .to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_plaintext_agent_message(
    message: String,
) -> Result<String, FunctionCallError> {
    let message = message_content(message)?;
    if message.contains(OMITTED_ENCRYPTED_PAYLOAD_STUB) {
        return Err(FunctionCallError::RespondToModel(
            "external_agents message must be the full plaintext task; do not send the omitted encrypted-payload stub"
                .to_string(),
        ));
    }
    if message.len() > PLAINTEXT_MESSAGE_MAX_BYTES {
        return Err(FunctionCallError::RespondToModel(format!(
            "external_agents message exceeds the {PLAINTEXT_MESSAGE_MAX_BYTES} byte limit"
        )));
    }
    Ok(message)
}

pub(crate) fn plaintext_user_input(message: String) -> Vec<UserInput> {
    vec![UserInput::Text {
        text: message,
        text_elements: Vec::new(),
    }]
}

pub(crate) fn is_allowlisted_external_role(
    config: &Config,
    task_name: &str,
    agent_type: Option<&str>,
    model: Option<&str>,
) -> bool {
    if infer_muse_spark_role(task_name, agent_type, model).is_some() {
        return true;
    }
    let Some(role_name) = agent_type.map(str::trim).filter(|role| !role.is_empty()) else {
        return false;
    };
    role_selects_non_openai_provider(config, role_name)
}

pub(crate) fn agent_is_muse_spark(metadata: &AgentMetadata) -> bool {
    infer_muse_spark_role(
        metadata
            .agent_path
            .as_ref()
            .map(codex_protocol::AgentPath::name)
            .unwrap_or(""),
        metadata.agent_role.as_deref(),
        None,
    )
    .is_some()
        || metadata
            .agent_path
            .as_ref()
            .is_some_and(|path| looks_like_muse_spark_name(path.name()))
}

fn role_selects_non_openai_provider(config: &Config, role_name: &str) -> bool {
    let Some(provider_id) = peek_role_model_provider(config, role_name) else {
        return false;
    };
    if provider_id.eq_ignore_ascii_case("openai") {
        return false;
    }
    config
        .model_providers
        .get(&provider_id)
        .is_some_and(|provider| !provider.requires_openai_auth)
}

fn peek_role_model_provider(config: &Config, role_name: &str) -> Option<String> {
    let role = resolve_role_config(config, role_name)?;
    let path = role.config_file.as_ref()?;
    let contents = std::fs::read_to_string(path).ok()?;
    let table: toml::Value = contents.parse().ok()?;
    table
        .get("model_provider")
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
}
