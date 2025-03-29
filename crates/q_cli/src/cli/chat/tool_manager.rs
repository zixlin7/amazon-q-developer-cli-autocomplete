use std::collections::HashMap;
use std::hash::{
    DefaultHasher,
    Hash,
    Hasher,
};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use convert_case::Casing;
use crossterm::{
    queue,
    style,
};
use fig_api_client::model::{
    ToolResult,
    ToolResultContentBlock,
    ToolResultStatus,
};
use futures::{
    StreamExt,
    stream,
};
use serde::{
    Deserialize,
    Serialize,
};
use tracing::error;

use super::parser::ToolUse;
use super::tools::Tool;
use super::tools::custom_tool::{
    CustomToolClient,
    CustomToolConfig,
};
use super::tools::execute_bash::ExecuteBash;
use super::tools::fs_read::FsRead;
use super::tools::fs_write::FsWrite;
use super::tools::gh_issue::GhIssue;
use super::tools::use_aws::UseAws;
use crate::cli::chat::tools::ToolSpec;
use crate::cli::chat::tools::custom_tool::CustomTool;

const NAMESPACE_DELIMITER: &str = "___";
// This applies for both mcp server and tool name since in the end the tool name as seen by the
// model is just {server_name}{NAMESPACE_DELIMITER}{tool_name}
const VALID_TOOL_NAME: &str = "[a-zA-Z][a-zA-Z0-9_]*";

// This is to mirror claude's config set up
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    mcp_servers: HashMap<String, CustomToolConfig>,
}

impl McpServerConfig {
    pub async fn load_config(output: &mut impl Write) -> eyre::Result<Self> {
        let mut cwd = std::env::current_dir()?;
        cwd.push(".amazonq/mcp.json");
        let expanded_path = shellexpand::tilde("~/.aws/amazonq/mcp.json");
        let global_path = PathBuf::from(expanded_path.as_ref());
        let global_buf = tokio::fs::read(global_path).await.ok();
        let local_buf = tokio::fs::read(cwd).await.ok();
        let conf = match (global_buf, local_buf) {
            (Some(global_buf), Some(local_buf)) => {
                let mut global_conf = serde_json::from_slice::<Self>(&global_buf)?;
                let local_conf = serde_json::from_slice::<Self>(&local_buf)?;
                for (server_name, config) in local_conf.mcp_servers {
                    if global_conf.mcp_servers.insert(server_name.clone(), config).is_some() {
                        queue!(
                            output,
                            style::SetForegroundColor(style::Color::Yellow),
                            style::Print("WARNING: "),
                            style::ResetColor,
                            style::Print("MCP config conflict for "),
                            style::SetForegroundColor(style::Color::Green),
                            style::Print(server_name),
                            style::ResetColor,
                            style::Print(". Using workspace version.\n")
                        )?;
                    }
                }
                global_conf
            },
            (None, Some(local_buf)) => serde_json::from_slice::<Self>(&local_buf)?,
            (Some(global_buf), None) => serde_json::from_slice::<Self>(&global_buf)?,
            _ => Default::default(),
        };
        output.flush()?;
        Ok(conf)
    }
}

#[derive(Default)]
pub struct ToolManager {
    clients: HashMap<String, Arc<CustomToolClient>>,
}

impl ToolManager {
    pub async fn from_configs(config: McpServerConfig) -> eyre::Result<Self> {
        let McpServerConfig { mcp_servers } = config;
        let regex = regex::Regex::new(VALID_TOOL_NAME)?;
        let mut hasher = DefaultHasher::new();
        let pre_initialized = mcp_servers
            .into_iter()
            .map(|(server_name, server_config)| {
                let server_name = {
                    let snake_case = server_name.to_case(convert_case::Case::Snake);
                    sanitize_server_name(snake_case, &regex, &mut hasher)
                };
                let custom_tool_client = CustomToolClient::from_config(server_name.clone(), server_config);
                (server_name, custom_tool_client)
            })
            .collect::<Vec<(String, _)>>();
        let init_results = stream::iter(pre_initialized)
            .map(|(name, uninit_client)| async move { (name, uninit_client.await) })
            .buffer_unordered(10)
            .collect::<Vec<(String, _)>>()
            .await;
        let mut clients = HashMap::<String, Arc<CustomToolClient>>::new();
        for (mut name, init_res) in init_results {
            match init_res {
                Ok(client) => {
                    let mut client = Arc::new(client);
                    while let Some(collided_client) = clients.insert(name.clone(), client) {
                        // to avoid server name collision we are going to circumvent this by
                        // appending the name with 1
                        name.push('1');
                        client = collided_client;
                    }
                },
                Err(e) => {
                    error!("Error initializing for mcp client {}: {:?}", name, e);
                },
            }
        }
        Ok(Self { clients })
    }

    pub async fn load_tools(&self) -> eyre::Result<HashMap<String, ToolSpec>> {
        let mut tool_specs = serde_json::from_str::<HashMap<String, ToolSpec>>(include_str!("tools/tool_index.json"))?;
        let load_tool = self
            .clients
            .iter()
            .map(|(server_name, client)| {
                let client_clone = client.clone();
                let server_name_clone = server_name.clone();
                async move { (server_name_clone, client_clone.get_tool_spec().await) }
            })
            .collect::<Vec<_>>();
        let load_tool_results = stream::iter(load_tool)
            .map(|async_closure| tokio::task::spawn(async_closure))
            .buffer_unordered(20)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|item| item.ok())
            .collect::<Vec<(String, _)>>();
        for (server_name, load_tool_result) in load_tool_results {
            match load_tool_result {
                Ok((name, specs)) => {
                    // Each mcp server might have multiple tools.
                    // To avoid naming conflicts we are going to namespace it.
                    // This would also help us locate which mcp server to call the tool from.
                    for mut spec in specs {
                        spec.name = format!("{}{}{}", name, NAMESPACE_DELIMITER, spec.name);
                        tool_specs.insert(spec.name.clone(), spec);
                    }
                },
                Err(e) => {
                    error!("Error obtaining tool spec for {}: {:?}", server_name, e);
                },
            }
        }
        Ok(tool_specs)
    }

    pub fn get_tool_from_tool_use(&self, value: ToolUse) -> Result<Tool, ToolResult> {
        let map_err = |parse_error| ToolResult {
            tool_use_id: value.id.clone(),
            content: vec![ToolResultContentBlock::Text(format!(
                "Failed to validate tool parameters: {parse_error}. The model has either suggested tool parameters which are incompatible with the existing tools, or has suggested one or more tool that does not exist in the list of known tools."
            ))],
            status: ToolResultStatus::Error,
        };

        Ok(match value.name.as_str() {
            "fs_read" => Tool::FsRead(serde_json::from_value::<FsRead>(value.args).map_err(map_err)?),
            "fs_write" => Tool::FsWrite(serde_json::from_value::<FsWrite>(value.args).map_err(map_err)?),
            "execute_bash" => Tool::ExecuteBash(serde_json::from_value::<ExecuteBash>(value.args).map_err(map_err)?),
            "use_aws" => Tool::UseAws(serde_json::from_value::<UseAws>(value.args).map_err(map_err)?),
            "report_issue" => Tool::GhIssue(serde_json::from_value::<GhIssue>(value.args).map_err(map_err)?),
            // Note that this name is namespaced with server_name{DELIMITER}tool_name
            name => {
                let (server_name, tool_name) = name.split_once(NAMESPACE_DELIMITER).ok_or(ToolResult {
                    tool_use_id: value.id.clone(),
                    content: vec![ToolResultContentBlock::Text(format!(
                        "The tool, \"{name}\" is supplied with incorrect name"
                    ))],
                    status: ToolResultStatus::Error,
                })?;
                let Some(client) = self.clients.get(server_name) else {
                    return Err(ToolResult {
                        tool_use_id: value.id,
                        content: vec![ToolResultContentBlock::Text(format!(
                            "The tool, \"{server_name}\" is not supported by the client"
                        ))],
                        status: ToolResultStatus::Error,
                    });
                };
                // The tool input schema has the shape of { type, properties }.
                // The field "params" expected by MCP is { name, arguments }, where name is the
                // name of the tool being invoked,
                // https://spec.modelcontextprotocol.io/specification/2024-11-05/server/tools/#calling-tools.
                // The field "arguments" is where ToolUse::args belong.
                let mut params = serde_json::Map::<String, serde_json::Value>::new();
                params.insert("name".to_owned(), serde_json::Value::String(tool_name.to_owned()));
                params.insert("arguments".to_owned(), value.args);
                let params = serde_json::Value::Object(params);
                let custom_tool = CustomTool {
                    name: tool_name.to_owned(),
                    client: client.clone(),
                    method: "tools/call".to_owned(),
                    params: Some(params),
                };
                Tool::Custom(custom_tool)
            },
        })
    }
}

fn sanitize_server_name(orig: String, regex: &regex::Regex, hasher: &mut impl Hasher) -> String {
    if regex.is_match(&orig) {
        return orig;
    }
    let sanitized: String = orig.chars().filter(|c| regex.is_match(&c.to_string())).collect();
    if sanitized.is_empty() {
        orig.hash(hasher);
        hasher.finish().to_string()
    } else {
        sanitized
    }
}
