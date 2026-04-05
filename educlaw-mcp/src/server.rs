// educlaw-mcp/src/server.rs
//
// ServerHandler implementation that aggregates all tool modules.
// Follows the same shape as memorymcp OptmServer.

use std::borrow::Cow;
use std::sync::Arc;

use rmcp::{
    handler::server::ServerHandler,
    model::*,
    service::{RequestContext, RoleServer},
    ErrorData as McpError,
};
use tracing::{error, info};

use crate::config::{Config, StudentInfo};
use crate::embedding::EmbeddingClient;
use crate::search_index::SearchIndex;
use crate::servers::{class_files, eval_engine, shell_ns, student_context, student_files, teacher_broadcast};

// ── Server state ─────────────────────────────────────────────

#[derive(Clone)]
pub struct EduclawMcpServer {
    index: Arc<SearchIndex>,
    embedding: Arc<EmbeddingClient>,
    classes_root: String,
    students_root: String,
    student: Option<StudentInfo>,
    config: Arc<Config>,
}

impl EduclawMcpServer {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let index = Arc::new(SearchIndex::open(&config.search_db, config.embedding_dim).await?);
        let embedding = Arc::new(EmbeddingClient::new(config.embedding_url.clone()));

        // Best-effort health check — don't fail startup if embedding server
        // is not yet running; it may be started later.
        if let Err(e) = embedding.health_check().await {
            tracing::warn!("embedding server not reachable: {e} — semantic search will fail until it is started");
        }

        let student = config.student_info();
        Ok(Self {
            index,
            embedding,
            classes_root: config.classes_root.clone(),
            students_root: config.students_root.clone(),
            student,
            config: Arc::new(config),
        })
    }

    fn all_tools(&self) -> Vec<Tool> {
        let mut tools = Vec::new();
        tools.extend(class_files::tools());
        tools.extend(eval_engine::tools());
        tools.extend(shell_ns::tools());
        tools.extend(student_context::tools());
        tools.extend(student_files::tools());
        tools.extend(teacher_broadcast::tools());
        tools
    }

    async fn dispatch_tool(
        &self,
        name: &str,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<CallToolResult, McpError> {
        match name {
            // ClassFiles
            "list_class_files" | "read_class_file" | "search_class_content"
            | "index_class_file" => {
                class_files::call_tool(
                    name,
                    args,
                    &self.classes_root,
                    &self.index,
                    &self.embedding,
                    &self.student,
                )
                .await
            }
            // StudentContext
            "get_student_context" => {
                student_context::call_tool(name, args, &self.student)
            }
            // StudentFiles
            "list_student_files" | "read_student_file" | "write_student_file" => {
                student_files::call_tool(name, args, &self.students_root, &self.student)
            }
            // EvalEngine
            "list_assignments" | "get_assignment" | "check_answer"
            | "record_answer" | "get_progress" | "submit_assignment" => {
                eval_engine::call_tool(
                    name,
                    args,
                    &self.classes_root,
                    &self.students_root,
                    &self.student,
                )
            }
            // Shell-in-namespace
            "run_command" | "run_python" => {
                shell_ns::call_tool(name, args, &self.config).await
            }
            // TeacherBroadcast
            "broadcast_message" => {
                teacher_broadcast::call_tool(name, args, &self.student, &self.config).await
            }
            _ => Err(McpError::invalid_params(
                format!("unknown tool: {name}"),
                None,
            )),
        }
    }
}

// ── rmcp ServerHandler ────────────────────────────────────────────────────

impl ServerHandler for EduclawMcpServer {
    async fn ping(
        &self,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        Ok(())
    }

    async fn initialize(
        &self,
        request: InitializeRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        info!("MCP client initialising");
        Ok(InitializeResult {
            protocol_version: request.protocol_version,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability::default()),
                ..Default::default()
            },
            server_info: Implementation {
                name: "educlaw-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                title: None,
                website_url: None,
            },
            instructions: Some(
                "EduClaw MCP server — provides class materials, student context, \
                 and semantic search over course content."
                    .to_string(),
            ),
        })
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools = self.all_tools();
        info!("list_tools → {} tools", tools.len());
        Ok(ListToolsResult { tools, next_cursor: None, meta: None })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        info!("call_tool: {}", request.name);

        let args = request
            .arguments
            .as_ref()
            .ok_or_else(|| McpError::invalid_params("arguments required", None))?;

        match self.dispatch_tool(&request.name, args).await {
            Ok(result) => Ok(result),
            Err(e) => {
                error!("tool '{}' failed: {:?}", request.name, e);
                Err(e)
            }
        }
    }
}
