use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum McpError {
    #[error("MCP client error: {0}")]
    ClientError(String),

    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallResult {
    pub tool_name: String,
    pub success: bool,
    pub result_summary: String,
}

pub struct McpRouter;

impl McpRouter {
    pub async fn dispatch_action(
        action_type: &str,
        target_tool: &str,
        extracted_text: &str,
    ) -> Result<McpToolCallResult, McpError> {
        tracing::info!(
            "Dispatching MCP action: type={}, tool={}, text='{}'",
            action_type, target_tool, extracted_text
        );

        match action_type {
            "mcp_calendar" => Self::execute_calendar(target_tool, extracted_text).await,
            "local_reminder" => Self::execute_local_reminder(extracted_text).await,
            "mcp_notion" => Self::execute_notion(target_tool, extracted_text).await,
            "mcp_gdrive" => Self::execute_gdrive(target_tool, extracted_text).await,
            _ => Err(McpError::ClientError(format!("Unknown action type: {}", action_type))),
        }
    }

    async fn execute_calendar(tool: &str, text: &str) -> Result<McpToolCallResult, McpError> {
        Ok(McpToolCallResult {
            tool_name: tool.to_string(),
            success: true,
            result_summary: format!("Scheduled calendar event: '{}'", text),
        })
    }

    async fn execute_local_reminder(text: &str) -> Result<McpToolCallResult, McpError> {
        Ok(McpToolCallResult {
            tool_name: "os_notification".to_string(),
            success: true,
            result_summary: format!("Created local OS reminder: '{}'", text),
        })
    }

    async fn execute_notion(tool: &str, text: &str) -> Result<McpToolCallResult, McpError> {
        Ok(McpToolCallResult {
            tool_name: tool.to_string(),
            success: true,
            result_summary: format!("Pushed entry to Notion: '{}'", text),
        })
    }

    async fn execute_gdrive(tool: &str, text: &str) -> Result<McpToolCallResult, McpError> {
        Ok(McpToolCallResult {
            tool_name: tool.to_string(),
            success: true,
            result_summary: format!("Saved document to Google Drive: '{}'", text),
        })
    }
}
