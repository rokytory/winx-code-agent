use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::fmt::format::FmtSpan;
use winx::integrations::openai::{OpenAIClient, OpenAIConfig, OpenAIThinking};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// API Key for OpenAI (defaults to OPENAI_API_KEY env var)
    #[arg(short, long)]
    api_key: Option<String>,

    /// Organization ID for OpenAI (defaults to OPENAI_ORG_ID env var)
    #[arg(short, long)]
    org_id: Option<String>,

    /// Model to use
    #[arg(short, long, default_value = "gpt-4o")]
    model: String,

    /// Maximum tokens to generate
    #[arg(short = 'k', long, default_value_t = 2048)]
    max_tokens: i32,

    /// Temperature (0.0 to 1.0)
    #[arg(short, long, default_value_t = 0.7)]
    temperature: f32,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a simple prompt
    Prompt {
        /// The prompt text
        prompt: String,
    },

    /// Run sequential thinking on a problem
    Think {
        /// The query to think about
        query: String,

        /// Number of thinking steps
        #[arg(short, long, default_value_t = 3)]
        steps: usize,

        /// Allow revisions to previous thoughts
        #[arg(short, long)]
        revisions: bool,

        /// Maximum number of revisions allowed
        #[arg(short = 'm', long, default_value_t = 1)]
        max_revisions: usize,

        /// System prompt to use
        #[arg(short = 'p', long)]
        system_prompt: Option<String>,

        /// Output file to save thinking results
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run SQL queries with thinking
    SqlThink {
        /// The SQL query or problem
        query: String,

        /// Number of thinking steps
        #[arg(short, long, default_value_t = 3)]
        steps: usize,

        /// Database file to use (if any)
        #[arg(short, long)]
        database: Option<PathBuf>,
    },
}

async fn run_cli() -> Result<()> {
    let cli = Cli::parse();

    // Configure logging
    let log_level = if cli.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_span_events(FmtSpan::CLOSE)
        .init();

    // Configure OpenAI client
    let config = OpenAIConfig {
        api_key: cli.api_key,
        org_id: cli.org_id,
        model: cli.model,
        max_tokens: Some(cli.max_tokens),
        temperature: Some(cli.temperature),
    };

    let client = OpenAIClient::new(Some(config)).context("Failed to create OpenAI client")?;

    // Execute command
    match &cli.command {
        Commands::Prompt { prompt } => {
            let response = client.execute_prompt(prompt).await?;
            println!("{}", response);
        }

        Commands::Think {
            query,
            steps,
            revisions,
            max_revisions,
            system_prompt,
            output,
        } => {
            let mut thinking = OpenAIThinking::new(client, system_prompt.clone());

            let result = if *revisions {
                thinking
                    .process_query_with_revisions(query, *steps, *max_revisions)
                    .await?
            } else {
                thinking.process_query(query, *steps).await?
            };

            // Print the result
            println!("{}", result);

            // Save to file if requested
            if let Some(path) = output {
                std::fs::write(path, &result).context("Failed to write output to file")?;
                info!("Results saved to {:?}", path);
            }
        }

        Commands::SqlThink {
            query,
            steps,
            database,
        } => {
            // Create system prompt for SQL thinking
            let system_prompt = format!(
                "You are an expert SQL analyst that thinks step by step to solve database problems. {}",
                if let Some(db_path) = database {
                    format!("You are working with the database located at {:?}.", db_path)
                } else {
                    "You will design and suggest SQL queries based on the problem description.".to_string()
                }
            );

            let mut thinking = OpenAIThinking::new(client, Some(system_prompt));

            // Process the query
            let result = thinking.process_query(query, *steps).await?;

            // Print the result
            println!("{}", result);

            // If a database was specified, extract and execute SQL queries
            if let Some(db_path) = database {
                println!("\n--- SQL Execution ---");

                // Initialize SQL connection
                let conn = winx::sql::DbConnection::open(Some(db_path))?;

                // Extract SQL queries from the result (simplified approach)
                let sql_queries = extract_sql_queries(&result);

                for (i, query) in sql_queries.iter().enumerate() {
                    println!("\nExecuting SQL Query #{}: {}", i + 1, query);

                    match conn.query(query) {
                        Ok(results) => {
                            // Format and print results
                            for row in &results {
                                println!("{:?}", row);
                            }
                            println!("\n{} rows returned", results.len());
                        }
                        Err(e) => {
                            println!("Error executing query: {}", e);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Extract SQL queries from text
fn extract_sql_queries(text: &str) -> Vec<String> {
    let mut queries = Vec::new();
    let mut in_code_block = false;
    let mut current_query = String::new();

    for line in text.lines() {
        if line.contains("```sql") {
            in_code_block = true;
            continue;
        } else if line.contains("```") && in_code_block {
            in_code_block = false;
            if !current_query.trim().is_empty() {
                queries.push(current_query.trim().to_string());
                current_query = String::new();
            }
            continue;
        }

        if in_code_block {
            current_query.push_str(line);
            current_query.push('\n');
        }
    }

    queries
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize Winx
    winx::init()?;

    // Run CLI
    run_cli().await?;

    Ok(())
}
