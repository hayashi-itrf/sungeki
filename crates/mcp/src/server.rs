use crate::{context::Context, dispatch, tools};
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use sungeki_host_client::{HostClient, HostOptions};
use sungeki_protocol::EmptyArgs;
use tokio::sync::Mutex;

#[derive(Default)]
pub struct BootstrapServer {
    context: Mutex<Context>,
}
fn encode<T: serde::Serialize>(value: T) -> Result<serde_json::Value, ErrorData> {
    serde_json::to_value(value).map_err(|e| ErrorData::internal_error(e.to_string(), None))
}
impl ServerHandler for BootstrapServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("sungeki-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions("create_projectまたはattach_sessionで対象を選び、apply_commandsで共通ホストを編集します。save_projectで文書を保存できます。台本からの一括制作はbuild_videoで開始し、返された同じjob_idをget_build_videoで追跡してください。既存作品の動画出力はstart_render_jobとget_render_jobを使います。必要な実行環境と素材を確認してから開始し、確認レポートの未検査項目を合格と扱わないでください。")
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let mut result = ListToolsResult::default();
        result.tools = tools::definitions();
        Ok(result)
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let args = serde_json::Value::Object(request.arguments.unwrap_or_default());
        let mut context = self.context.lock().await;
        let command = if request.name == "get_capabilities" {
            serde_json::from_value::<EmptyArgs>(args)
                .map_err(|e| ErrorData::invalid_params(e.to_string(), None))?;
            None
        } else {
            Some(dispatch::request(&request.name, args, &mut context)?)
        };
        let result = async {
            let mut client =
                HostClient::connect_or_start(&HostOptions::from_environment()?, "MCP").await?;
            if let Some((command, attach)) = command {
                let response = client.session(command).await?;
                context.observe(&response, attach);
                Ok(serde_json::to_value(response).expect("typed response is serializable"))
            } else {
                Ok(serde_json::to_value(client.capabilities().await?)
                    .expect("typed capabilities are serializable"))
            }
        }
        .await;
        let output = match result {
            Ok(value) => CallToolResult::structured(value),
            Err(error) => {
                CallToolResult::structured_error(encode::<sungeki_protocol::ApiError>(error)?)
            }
        };
        Ok(output.into())
    }
}
