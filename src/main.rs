use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "olly", about = "Rust TUI client for Ollama")]
struct Cli {
    #[arg(long)]
    url: Option<String>,
    #[arg(long)]
    model: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    olly::app::run(cli.url, cli.model).await?;
    Ok(())
}
