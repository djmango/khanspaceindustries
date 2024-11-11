use anyhow::{Context, Result};
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs,
};
use async_openai::Client;
use chrono::Local;
use dotenv::dotenv;
use regex::Regex;
use std::env;
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::Path;
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let api_key = env::var("OPENAI_API_KEY").context("Missing OPENAI_API_KEY")?;
    let base_directory = "./Experiments/";

    let client = Client::with_config(OpenAIConfig::new().with_api_key(api_key));
    let folder_pattern =
        Regex::new(r"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d{4}$")?;

    for entry in fs::read_dir(base_directory).context("Failed to read base directory")? {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(folder_name) = path.file_name().and_then(|n| n.to_str()) {
                if folder_pattern.is_match(folder_name) {
                    let markdown_file = path.join(format!("{}_summary.md", folder_name));
                    if markdown_file.exists() {
                        println!("Summary already exists for {}", folder_name);
                        continue;
                    }

                    println!("Processing folder: {}", folder_name);
                    let summaries = process_experiment_files(&path, &client).await?;
                    let experiment_count = summaries.len();

                    if experiment_count > 0 {
                        println!(
                            "Found {} experiment transcript(s) in {}. Generating summary...",
                            experiment_count, folder_name
                        );
                        let markdown_content = create_markdown_document(folder_name, &summaries);
                        let mut file = File::create(&markdown_file).with_context(|| {
                            format!("Failed to create summary file: {}", markdown_file.display())
                        })?;
                        file.write_all(markdown_content.as_bytes())?;
                        println!("Generated summary for {}", folder_name);
                    } else {
                        println!("No transcripts found in {}", folder_name);
                    }
                } else {
                    println!(
                        "Skipping folder: {} (does not match expected format)",
                        folder_name
                    );
                }
            }
        }
    }

    Ok(())
}

async fn process_experiment_files(
    directory: &Path,
    client: &Client<OpenAIConfig>,
) -> Result<Vec<String>> {
    let mut summaries = Vec::new();
    for entry in fs::read_dir(directory).context("Failed to read experiment directory")? {
        let entry = entry.context("Failed to read file entry")?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("txt") {
            let transcript = read_file_to_string(&path)?;
            println!("Sending request for transcript: {}", path.display());
            let summary = generate_summary(&transcript, client).await?;
            println!("Received summary for transcript: {}", path.display());
            summaries.push(summary);
        }
    }
    Ok(summaries)
}

fn read_file_to_string(path: &Path) -> Result<String> {
    let file =
        File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = String::new();
    buf_reader
        .read_to_string(&mut contents)
        .context("Failed to read file contents")?;
    Ok(contents)
}

async fn generate_summary(transcript: &str, client: &Client<OpenAIConfig>) -> Result<String> {
    let template_path = Path::new("template.md");
    let template = fs::read_to_string(template_path).context("Failed to read template.md")?;

    let prompt = format!(
        "You are a helpful lab assistant. Your task is to analyze and summarize experiment transcripts. \
        Use the following Markdown template for the summary:\n\n\
        {}\n\n\
        Now, based on this template, analyze and summarize the following experiment transcript:\n\n\
        {}",
        template, transcript
    );

    let messages = vec![ChatCompletionRequestMessage::User(
        ChatCompletionRequestUserMessage {
            content: ChatCompletionRequestUserMessageContent::Text(prompt),
            name: None,
        },
    )];

    println!("Sending chat completion request...");
    let request = CreateChatCompletionRequestArgs::default()
        .model("o1-mini")
        .messages(messages)
        .build()
        .context("Failed to build chat completion request")?;

    let response = client
        .chat()
        .create(request)
        .await
        .context("API request failed")?;
    let summary = response
        .choices
        .get(0)
        .and_then(|choice| choice.message.content.clone())
        .unwrap_or_else(|| "No summary generated.".to_string());

    Ok(summary.trim().to_string())
}

fn create_markdown_document(date: &str, summaries: &[String]) -> String {
    let mut markdown_content = format!("# Daily Experiment Summary - {}\n\n", date);
    for (i, summary) in summaries.iter().enumerate() {
        markdown_content.push_str(&format!(
            "## Experiment {}\n\n{}\n\n---\n\n",
            i + 1,
            summary
        ));
    }
    let generation_date = Local::now().format("%Y-%m-%d").to_string();
    markdown_content.push_str(&format!("*Generated on {}*", generation_date));
    markdown_content
}
