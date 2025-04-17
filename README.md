# Winx

Uma agente de código de alta performance escrita em Rust, combinando os melhores recursos do WCGW e do Serena para
máxima eficiência e capacidades semânticas.

## Funcionalidades

- ⚡ **Alto Desempenho**: Implementada em Rust para garantir velocidade e eficiência
- ⚡ **Análise Semântica de Código**: Integração com Language Server Protocol (LSP) para compreensão de símbolos de
  código
- ⚡ **Edição de arquivos otimizada**: Implementação eficiente de diff, edit e insert com algoritmos otimizados
- ⚡ **Memória de Projeto**: Sistema de memória inspirado no Serena para manter contexto entre sessões
- ⚡ **Pensamento Sequencial Avançado**: Ferramentas de raciocínio sobre aderência e conclusão de tarefas
- ⚡ **Verificação de Sintaxe**: Validação da sintaxe de código antes de aplicar modificações
- ⚡ **Suporte a SQL**: Interface integrada para executar consultas SQL
- ⚡ **Integração MCP**: Funciona como servidor MCP para Claude e outros LLMs
- ⚡ **Terminal Interativo**: Suporte para comandos interativos com feedback em tempo real
- ⚡ **Múltiplos modos de operação**: Suporte para `wcgw`, `architect` e `code_writer`
- ⚡ **Manipulação de Arquivos Grandes**: Edição incremental de arquivos grandes para evitar problemas de limites de
  tokens

## Instalação

Para compilar o projeto:

```bash
git clone https://github.com/your-username/winx.git
cd winx
cargo build --release
```

## Uso

```bash
./target/release/winx [workspace_path]
```

Se nenhum caminho for fornecido, o diretório atual será usado como workspace.

## Integração com Claude

Para integrar com Claude Desktop, configure o arquivo `claude_desktop_config.json` (localizado em
`~/Library/Application Support/Claude/claude_desktop_config.json` no macOS):

```json
{
  "mcpServers": {
    "winx": {
      "command": "/caminho/completo/para/winx",
      "args": []
    }
  }
}
```

## Ferramentas Disponíveis

Winx oferece as seguintes ferramentas para interação com o sistema:

- **BashCommand**: Execute comandos shell com suporte a interatividade
- **ReadFiles**: Leia conteúdo de um ou mais arquivos
- **FileWriteOrEdit**: Escreva ou edite arquivos com suporte a edições parciais
- **SqlQuery**: Execute consultas SQL interativamente
- **SequentialThinking**: Processador de pensamento sequencial para resolução de problemas
- **SymbolTools**: Ferramentas para manipulação de símbolos de código (inspiradas no Serena)
- **MemoryTools**: Ferramentas para guardar e recuperar memórias de projeto
- **TaskAdherence**: Ferramentas para avaliar a aderência e conclusão de tarefas
- **InteractiveTerminal**: Terminal interativo para comandos com entrada/saída em tempo real

## Modos de Operação

- **wcgw**: Modo padrão com todas as permissões
- **architect**: Modo somente leitura para planejamento
- **code_writer**: Modo restrito para escrever código em caminhos específicos

## Segurança

- O agente verifica permissões de arquivo antes de qualquer operação
- Restrições configuráveis para comandos e caminhos
- Verificação de alterações antes de aplicar edições a arquivos
- Verificação de sintaxe para evitar código mal formado

## Contribuição

Contribuições são bem-vindas! Abra um PR ou issue para começar.

## Licença

MIT
