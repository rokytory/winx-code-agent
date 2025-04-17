<table style="width:100%" align="center" border="0">
  <tr>
    <td><img src="./.github/assets/fairy.png" alt="Winx" width="300"></td>
    <td><h1>✨ Ｗｉｎｘ Ａｇｅｎｔ ✨</h1></td>
  </tr>
</table>

<p align="center">
  <strong>✨ A high-performance code agent written in Rust, combining the best features of WCGW for maximum efficiency and semantic capabilities. 🦀</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/language-Rust-orange?style=flat&logo=rust" alt="Language" />
  <img src="https://img.shields.io/badge/license-MIT-blue?style=flat" alt="License" />
  <img src="https://img.shields.io/github/last-commit/gabrielmaialva33/winx-code-agent?style=flat" alt="Last Commit" >
  <img src="https://img.shields.io/badge/made%20by-Maia-15c3d6?style=flat" alt="Made by Maia" >
  <img src="https://github.com/gabrielmaialva33/winx-code-agent/actions/workflows/ci.yml/badge.svg" alt="CI Status" />
</p>

---

## 🌟 Features

- ⚡ **High Performance**: Implemented in Rust for speed and efficiency
- ⚡ **Semantic Code Analysis**: Integration with Language Server Protocol (LSP) for code symbol understanding
- ⚡ **Optimized File Editing**: Efficient diff, edit and insert with optimized algorithms
- ⚡ **Project Memory**: Memory system inspired by Serena to maintain context between sessions
- ⚡ **Advanced Sequential Thinking**: Tools for reasoning about task adherence and completion
- ⚡ **Syntax Validation**: Code syntax validation before applying modifications
- ⚡ **SQL Support**: Integrated interface for executing SQL queries
- ⚡ **MCP Integration**: Functions as an MCP server for Claude and other LLMs
- ⚡ **Interactive Terminal**: Support for interactive commands with real-time feedback
- ⚡ **Multiple Operation Modes**: Support for `wcgw`, `architect` and `code_writer` modes
- ⚡ **Large File Handling**: Incremental editing of large files to avoid token limit issues

---

## 🚀 Installation

To compile the project from source:

```bash
git clone https://github.com/gabrielmaialva33/winx-code-agent.git
cd winx
cargo build --release
```

For basic usage:

```bash
./target/release/winx [workspace_path]
```

If no path is provided, the current directory will be used as the workspace.

---

## 🔧 Integration with Claude

Winx is inspired by the [WCGW project](https://github.com/rusiaaman/wcgw) but reimplemented in Rust for enhanced performance. To integrate with Claude Desktop, configure the file `claude_desktop_config.json` (located in `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "winx": {
      "command": "/path/to/winx",
      "args": []
    }
  }
}
```

Then restart the Claude app. You should be able to see the MCP icon if everything is set up correctly.

---

## 🛠️ Available Tools

Winx offers the following tools for interaction with the system:

- **BashCommand**: Execute shell commands with support for interactivity
- **ReadFiles**: Read content from one or more files
- **FileWriteOrEdit**: Write or edit files with support for partial edits
- **SqlQuery**: Execute SQL queries interactively
- **SequentialThinking**: Sequential thought processor for problem solving
- **SymbolTools**: Tools for code symbol manipulation (inspired by Serena)
- **MemoryTools**: Tools for storing and retrieving project memories
- **TaskAdherence**: Tools for evaluating task adherence and completion
- **InteractiveTerminal**: Interactive terminal for commands with real-time I/O

---

## 🔀 Operation Modes

- **wcgw**: Default mode with all permissions
- **architect**: Read-only mode for planning
- **code_writer**: Restricted mode for writing code in specific paths

---

## 👨‍💻 Usage Examples

- Ask Claude to explore and understand your codebase
- Request code analysis and semantic understanding
- Have Claude edit files with optimized algorithms
- Execute SQL queries and analyze results
- Run commands with real-time feedback
- Implement the sequential thinking process for complex problems
- Validate syntax before applying code changes
- Work with large files incrementally to avoid token limits

---

## 🏷 Need Support or Assistance?

If you need help or have any questions about Winx, feel free to reach out via the following channels:

- [GitHub Issues](https://github.com/gabrielmaialva33/winx-code-agent/issues/new): Open a support issue on GitHub.
- Email: gabrielmaialva33@gmail.com

---

## ❣️ Support the Project

If you enjoy **Winx Agent** and want to support its development, consider:

- ⭐ Starring the repository on GitHub.
- 🍴 Forking the repository and contributing improvements.
- 📝 Sharing your experience with tutorials or articles.

Together, we can make **Winx Agent** even better!

---

## 🔐 Security

- The agent verifies file permissions before operations
- Configurable restrictions for commands and paths
- Verification of changes before applying file edits
- Syntax checking to prevent malformed code

---

## 🙏 Special Thanks

A huge thank you to [rusiaaman](https://github.com/rusiaaman) for the inspiring work on [WCGW](https://github.com/rusiaaman/wcgw), which served as a primary inspiration for this project. Winx reimplements many of WCGW's best features in Rust for enhanced performance while adding additional capabilities for semantic code understanding.

---

## 📜 License

MIT