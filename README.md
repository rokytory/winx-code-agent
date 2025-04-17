# Winx

A high-performance code agent written in Rust, based on the WCGW project but optimized for efficiency and speed.

## Features

- ⚡ **High Performance**: Implemented in Rust to ensure speed and efficiency
- ⚡ **Optimized file editing**: Efficient implementation of diff, edit, and insert with optimized algorithms
- ⚡ **SQL Support**: Integrated interface for executing SQL queries
- ⚡ **Sequential Thinking**: Implementation of efficient sequential thinking algorithms
- ⚡ **MCP Integration**: Works as an MCP server for Claude and other LLMs
- ⚡ **Multiple operation modes**: Support for `wcgw`, `architect`, and `code_writer`

## Installation

To compile the project:

```bash
git clone https://github.com/your-username/winx.git
cd winx
cargo build --release
```

## Usage

```bash
./target/release/winx [workspace_path]
```

If no path is provided, the current directory will be used as the workspace.

## Integration with Claude

To integrate with Claude Desktop, configure the `claude_desktop_config.json` file (located at
`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "winx": {
      "command": "/full/path/to/winx",
      "args": []
    }
  }
}
```

## Available Tools

Winx offers the following tools for interaction with the system:

- **BashCommand**: Execute shell commands with interaction support
- **ReadFiles**: Read content from one or more files
- **FileWriteOrEdit**: Write or edit files with support for partial edits
- **SqlQuery**: Execute SQL queries interactively
- **SequentialThinking**: Sequential thinking processor for problem solving

## Operation Modes

- **wcgw**: Default mode with all permissions
- **architect**: Read-only mode for planning
- **code_writer**: Restricted mode for writing code in specific paths

## Security

- The agent checks file permissions before any operation
- Configurable restrictions for commands and paths
- Verification of changes before applying edits to files

## Contribution

Contributions are welcome! Open a PR or issue to get started.

## License

MIT
