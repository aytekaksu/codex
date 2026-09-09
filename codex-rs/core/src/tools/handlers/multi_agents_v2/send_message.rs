use super::analytics::ToolCallAnalytics;
use super::message_tool::MessageDeliveryMode;
use super::message_tool::SendMessageArgs;
use super::message_tool::handle_message_string_tool;
use super::*;
use crate::tools::handlers::external_agents_spec::create_external_send_message_tool;
use crate::tools::handlers::multi_agents_spec::create_send_message_tool;
use crate::tools::handlers::multi_agents_v2::external_agents::AgentTransport;
use codex_tools::ToolSpec;

#[derive(Default)]
pub(crate) struct Handler {
    surface: AgentTransport,
}

impl Handler {
    pub(crate) fn collaboration() -> Self {
        Self {
            surface: AgentTransport::Collaboration,
        }
    }

    pub(crate) fn external() -> Self {
        Self {
            surface: AgentTransport::ExternalAgents,
        }
    }
}

impl ToolExecutor<ToolInvocation> for Handler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("send_message")
    }

    fn spec(&self) -> ToolSpec {
        match self.surface {
            AgentTransport::Collaboration => create_send_message_tool(),
            AgentTransport::ExternalAgents => create_external_send_message_tool(),
        }
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(async move {
            let mut analytics = ToolCallAnalytics::new(&invocation, CollabAgentTool::SendMessage);
            let result = self.handle_call(invocation, &mut analytics).await;
            analytics.finish(&result);
            result
        })
    }
}

impl Handler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
        analytics: &mut ToolCallAnalytics,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let arguments = function_arguments(invocation.payload.clone())?;
        let args: SendMessageArgs = parse_arguments(&arguments)?;
        handle_message_string_tool(
            invocation,
            MessageDeliveryMode::QueueOnly,
            args.target,
            args.message,
            analytics,
            self.surface,
        )
        .await
        .map(boxed_tool_output)
    }
}

impl CoreToolRuntime for Handler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }
}
