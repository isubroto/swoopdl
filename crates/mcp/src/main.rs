//! swoopdl-mcp: Swoopdl's MCP server. An AI assistant starts it and
//! speaks MCP with it on stdin and stdout; it hands what the assistant asks to the running app
//! (docs/ai-assistants.md).

use rmcp::ServiceExt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Started by hand in a terminal: say what it is instead of waiting silently.
    if let Some(arg) = std::env::args().nth(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("swoopdl-mcp {}", env!("CARGO_PKG_VERSION"))
            }
            _ => println!(
                "swoopdl-mcp: Swoopdl's MCP server, for AI \
                 assistants. The assistant starts it (no arguments) and talks to it on stdin and \
                 stdout. Setup: \
                 https://misterdownloadmanager.com/help/ai-assistants.html"
            ),
        }
        return Ok(());
    }
    let server = sdl_mcp::Server::new(std::sync::Arc::new(sdl_ipc::request));
    server
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
