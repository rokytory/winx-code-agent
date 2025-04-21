# Winx Code Agent

A Rust implementation of a code agent that provides tools for code manipulation, bash execution, and file operations.
This project is inspired by the Python WCGW (What Could Go Wrong) project and implements similar functionality using the
RMCP protocol.

## Features

- **Bash Command Execution**: Execute bash commands and interact with running processes
- **Screen Support**: Run long-running commands in background using `screen` sessions
- **File Operations**:
    - Read files with line range support and chunking for large files
    - Write new files with syntax validation
    - Edit existing files with search/replace functionality
    - Syntax checking for code files
- **Operational Modes**:
    - `wcgw`: Complete access to all features
    - `architect`: Read-only mode for planning and repository analysis
    - `code_writer`: Restricted access for controlled code modifications
- **Repository Analysis**: Analyze repository structure and get context information
- **Context Management**: Save task context and restore at a later time
- **Task Checkpointing**: Resume tasks where they were left off
- **Image Support**: Read images and encode them as base64 for display

## Requirements

- Rust 1.70 or higher
- Tokio runtime
- RMCP SDK

## Installation

### Clone the repository

```bash
git clone https://github.com/yourusername/winx-code-agent.git
cd winx-code-agent
```

### Build the project

```bash
cargo build
```

## Usage

### Running the server

```bash
cargo run
```

This will start the code agent server using stdio for communication.

### As a library

You can also use Winx Code Agent as a library in your Rust projects:

```rust
use anyhow::Result;
use rmcp::{transport::io, ServiceExt};
use winx_code_agent::CodeAgent;

#[tokio::main]
async fn main() -> Result<()> {
    let agent = CodeAgent::new();
    let transport = io::stdio();

    let server = agent.serve(transport).await?;
    server.waiting().await?;

    Ok(())
}
```

### Example Usage

Check the examples directory for more examples:

```bash
# Run basic example
cargo run --example basic_usage

# Run advanced example
cargo run --example advanced_usage
```

## API Reference

The code agent provides the following tools via the RMCP protocol:

### Initialize

- Always call this at the start of the conversation before using any of the shell tools.
- Use `any_workspace_path` to initialize the shell in the appropriate project directory.
- If the user has mentioned a workspace or project root or any other file or folder use it to set `any_workspace_path`.
- If user has mentioned any files use `initial_files_to_read` to read, use absolute paths only (~ allowed)
- By default use mode "wcgw"
- In "code-writer" mode, set the commands and globs which user asked to set, otherwise use 'all'.
- Use type="first_call" if it's the first call to this tool.
- Use type="user_asked_mode_change" if in a conversation user has asked to change mode.
- Use type="reset_shell" if in a conversation shell is not working after multiple tries.
- Use type="user_asked_change_workspace" if in a conversation user asked to change workspace

Parameters:

- `type`: Type of initialization ("first_call", "user_asked_mode_change", "reset_shell", "user_asked_change_workspace")
- `any_workspace_path`: Path to the workspace directory
- `initial_files_to_read`: List of files to read initially
- `task_id_to_resume`: ID of a task to resume (if any)
- `mode_name`: Mode to operate in ("wcgw", "architect", "code_writer")
- `code_writer_config`: Configuration for code_writer mode

### BashCommand

- Execute a bash command. This is stateful (beware with subsequent calls).
- Status of the command and the current working directory will always be returned at the end.
- The first or the last line might be `(...truncated)` if the output is too long.
- Always run `pwd` if you get any file or directory not found error to make sure you're not lost.
- Run long running commands in background using screen instead of "&".
- Do not use 'cat' to read files, use ReadFiles tool instead
- In order to check status of previous command, use `status_check` with empty command argument.
- Only command is allowed to run at a time. You need to wait for any previous command to finish before running a new
  one.
- Programs don't hang easily, so most likely explanation for no output is usually that the program is still running, and
  you need to check status again.
- Do not send Ctrl-c before checking for status till 10 minutes or whatever is appropriate for the program to finish.

Parameters:

- `action_json`: Action to perform (Command, StatusCheck, SendText, SendSpecials)
- `wait_for_seconds`: Time to wait for output

### ReadFiles

- Read full file content of one or more files.
- Provide absolute paths only (~ allowed)
- Only if the task requires line numbers understanding:
    - You may populate "show_line_numbers_reason" with your reason, by default null/empty means no line numbers are
      shown.
    - You may extract a range of lines. E.g., `/path/to/file:1-10` for lines 1-10. You can drop start or end like
      `/path/to/file:1-` or `/path/to/file:-10`

Parameters:

- `file_paths`: Paths of files to read (supports line ranges like "file.txt:10-20")
- `show_line_numbers_reason`: Reason for showing line numbers

### WriteIfEmpty

Create new files or write to empty files only.

Parameters:

- `file_path`: Path of the file to write
- `file_content`: Content to write to the file

### FileEdit

- Edits existing files using search/replace blocks.
- Uses Aider-like search and replace syntax.
- File edit has spacing tolerant matching, with warning on issues like indentation mismatch.
- If there's no match, the closest match is returned to help fix mistakes.

Parameters:

- `file_path`: Path of the file to edit
- `file_edit_using_search_replace_blocks`: Edit using search/replace blocks

### ReadImage

Read an image file and return its base64-encoded content.

Parameters:

- `file_path`: Path of the image file

### ContextSave

Saves provided description and file contents of all the relevant file paths or globs in a single text file.

- Provide random unqiue id or whatever user provided.
- Leave project path as empty string if no project path

Parameters:

- `id`: ID to assign to the saved context
- `project_root_path`: Root path of the project
- `description`: Description of the context
- `relevant_file_globs`: Glob patterns to match relevant files

## License

MIT
