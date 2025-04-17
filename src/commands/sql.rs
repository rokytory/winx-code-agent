use anyhow::Result;
use tracing::{debug, info};

use crate::core::state::SharedState;
use crate::sql::{DbConnection, execute_query, format_results_as_table};

/// Execute an SQL query
pub async fn execute_sql_query(state: &SharedState, query: &str) -> Result<String> {
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
