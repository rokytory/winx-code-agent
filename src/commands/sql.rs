use anyhow::{Context, Result};
use serde_json::Value;
use tracing::{debug, info};

use crate::core::state::SharedState;
use crate::sql::{DbConnection, execute_query, format_results_as_table};

/// Execute an SQL query from a JSON request
pub async fn execute_sql_query(_state: &SharedState, query_json: &str) -> Result<String> {
    debug!("Executing SQL query from JSON: {}", query_json);
    
    // Parse the JSON request
    let json: Value = serde_json::from_str(query_json)?;
    
    // Extract the SQL query
    let query = match json.get("sql") {
        Some(Value::String(q)) => q,
        _ => return Err(anyhow::anyhow!("Invalid or missing 'sql' field in JSON")),
    };
    
    // Execute the SQL query using the existing functionality
    execute_sql_query_internal(query).await
}

/// Internal implementation of the SQL query execution
async fn execute_sql_query_internal(query: &str) -> Result<String> {
    debug!("Executing SQL query: {}", query);
    
    // Create an in-memory database connection
    let conn = DbConnection::open(None::<&str>)?;
    
    // Execute the query
    let results = execute_query(&conn, query)?;
    
    // Format the results as a table
    let formatted = format_results_as_table(&results);
    
    info!("SQL query execution completed: {} rows returned", results.row_count);
    Ok(formatted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{state::create_shared_state, types::ModeType};
    use tokio::runtime::Runtime;
    use tempfile::tempdir;
    
    #[test]
    fn test_sql_query() {
        let rt = Runtime::new().unwrap();
        
        rt.block_on(async {
            let temp_dir = tempdir().unwrap();
            let state = create_shared_state(temp_dir.path(), ModeType::Wcgw, None, None).unwrap();
            
            // Create a test database and execute a query
            let query = "
                CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT);
                INSERT INTO test (id, name) VALUES (1, 'Alice');
                INSERT INTO test (id, name) VALUES (2, 'Bob');
                SELECT * FROM test;
            ";
            
            let result = execute_sql_query(&state, query).await.unwrap();
            
            // Verify the results
            assert!(result.contains("Alice"));
            assert!(result.contains("Bob"));
        });
    }
}
