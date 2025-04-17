# Winx com OpenAI - Pensamento Sequencial

Este documento explica como utilizar o módulo de Pensamento Sequencial do Winx integrado com a API OpenAI.

## Requisitos

Para utilizar a integração com a API OpenAI, você precisará:

1. Uma chave de API OpenAI (API Key)
2. Opcionalmente, um ID de organização (Org ID)

Você pode definir essas informações como variáveis de ambiente:

```bash
export OPENAI_API_KEY=sua-chave-api
export OPENAI_ORG_ID=seu-id-de-organizacao  # opcional
```

Ou fornecê-las diretamente na linha de comando.

## Executando o CLI

O binário `winx_openai` oferece várias funcionalidades para trabalhar com a API do OpenAI:

### Comando Simples

Para executar um prompt simples:

```bash
winx_openai prompt "Explique o conceito de recursão"
```

### Pensamento Sequencial

Para resolver um problema usando pensamento sequencial:

```bash
winx_openai think --steps 3 "Como resolver o problema das Torres de Hanói com 3 discos?"
```

Opções:
- `--steps`: Número de passos de pensamento (padrão: 3)
- `--revisions`: Permite revisões dos pensamentos anteriores
- `--max-revisions`: Número máximo de revisões (padrão: 1)
- `--system-prompt`: Prompt de sistema personalizado
- `--output`: Arquivo para salvar os resultados

### Pensamento Sequencial com SQL

Para resolver um problema de SQL usando pensamento sequencial:

```bash
winx_openai sql-think "Como eu posso consultar os clientes que fizeram mais de 3 compras no último mês?"
```

Com um banco de dados específico:

```bash
winx_openai sql-think --database ./meu_banco.db "Quantos usuários se registraram em março?"
```

## Opções Globais

Estas opções funcionam com qualquer comando:

- `--api-key`: Chave de API OpenAI (sobrepõe a variável de ambiente)
- `--org-id`: ID da organização OpenAI (sobrepõe a variável de ambiente)
- `--model`: Modelo a ser usado (padrão: "gpt-4o")
- `--max-tokens`: Máximo de tokens a serem gerados (padrão: 2048)
- `--temperature`: Temperatura de amostragem (0.0 a 1.0, padrão: 0.7)
- `--verbose`: Habilita logs detalhados

## Exemplos

### Resolver um problema matemático passo a passo

```bash
winx_openai think "Qual é a derivada de f(x) = x^3 + 2x^2 - 5x + 3?"
```

### Revisar o raciocínio para um problema complexo

```bash
winx_openai think --steps 5 --revisions "Quais são as implicações éticas da inteligência artificial generativa?"
```

### Analisar dados com SQL

```bash
winx_openai sql-think --database ./vendas.db "Identifique tendências de vendas sazonais nos últimos dois anos"
```

## Integração Programática

Você também pode usar a integração do OpenAI em seu próprio código:

```rust
use winx::integrations::openai::{OpenAIClient, OpenAIConfig, OpenAIThinking};

async fn example() -> anyhow::Result<()> {
    // Configure o cliente
    let config = OpenAIConfig {
        api_key: None, // Usar variável de ambiente
        org_id: None,
        model: "gpt-4o".to_string(),
        max_tokens: Some(2048),
        temperature: Some(0.7),
    };
    
    let client = OpenAIClient::new(Some(config))?;
    
    // Crie o módulo de pensamento sequencial
    let mut thinking = OpenAIThinking::new(client, None);
    
    // Processe uma consulta
    let result = thinking.process_query(
        "Como desenvolver um algoritmo eficiente para encontrar números primos?", 
        3
    ).await?;
    
    println!("{}", result);
    
    Ok(())
}
```

## Integração com agentes de código

O módulo de pensamento sequencial do OpenAI pode ser utilizado em conjunto com os recursos de automação de código do Winx para criar agentes de código mais robustos e que aplicam pensamento passo a passo para resolver problemas complexos.
