use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::connection::DbConnection;

/// Represents a query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Column names
    pub columns: Vec<String>,
    /// Rows of data
    pub rows: Vec<Vec<String>>,
    /// Number of rows affected or returned
    pub row_count: usize,
}

/// Execute a query and format the results
pub fn execute_query(conn: &DbConnection, sql: &str) -> Result<QueryResult> {
    debug!("Executing SQL query: {}", sql);

    // First, we need to get the column names
    let columns = get_column_names(conn, sql)?;

    // Execute the query and get the results
    let rows = conn.query(sql)?;
    let row_count = rows.len();

    Ok(QueryResult {
        columns,
        rows,
        row_count,
    })
}

/// Get column names from a query
fn get_column_names(conn: &DbConnection, sql: &str) -> Result<Vec<String>> {
    // Check if this is a CREATE TABLE or other DDL statement
    let sql_upper = sql.trim().to_uppercase();
    if sql_upper.starts_with("CREATE ") || sql_upper.starts_with("DROP ") || sql_upper.starts_with("ALTER ") {
        // For DDL statements, just return placeholder column names
        return Ok(vec!["Result".to_string()]);
    }

    // For SELECT statements, we can use the LIMIT 0 approach
    if sql_upper.starts_with("SELECT ") {
        // We use a trick to get column names without executing the full query
        // by wrapping it in a LIMIT 0 query
        let limit_sql = format!("SELECT * FROM ({}) LIMIT 0", sql);

        match conn.query(&limit_sql) {
            Ok(connection) => {
                // If we got no results but no error, return default columns
                if connection.is_empty() {
                    return Ok(vec!["Column1".to_string(), "Column2".to_string()]);
                }
            }
            Err(_) => {
                // If the LIMIT 0 approach fails, just use the original query
                let results = conn.query(sql)?;
                if results.is_empty() {
                    return Ok(vec!["Column1".to_string(), "Column2".to_string()]);
                }
            }
        }
    }

    // TODO: Actually get column names from rusqlite - for now return placeholders
    Ok(vec!["Column1".to_string(), "Column2".to_string()])
}

/// Format query results as a table string
pub fn format_results_as_table(result: &QueryResult) -> String {
    if result.rows.is_empty() {
        return "No results found.".to_string();
    }

    // Calculate column widths
    let mut col_widths = result
        .columns
        .iter()
        .map(|col| col.len())
        .collect::<Vec<_>>();

    for row in &result.rows {
        for (i, cell) in row.iter().enumerate() {
            if i < col_widths.len() {
                col_widths[i] = col_widths[i].max(cell.len());
            }
        }
    }

    // Format header
    let mut output = String::new();

    // Header row
    for (i, col) in result.columns.iter().enumerate() {
        if i > 0 {
            output.push_str(" | ");
        }
        output.push_str(&format!("{:width$}", col, width = col_widths[i]));
    }
    output.push('\n');

    // Separator row
    for (i, width) in col_widths.iter().enumerate() {
        if i > 0 {
            output.push_str("-+-");
        }
        output.push_str(&"-".repeat(*width));
    }
    output.push('\n');

    // Data rows
    for row in &result.rows {
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                output.push_str(" | ");
            }
            if i < col_widths.len() {
                output.push_str(&format!("{:width$}", cell, width = col_widths[i]));
            } else {
                output.push_str(cell);
            }
        }
        output.push('\n');
    }

    // Add row count
    output.push_str(&format!("\n{} row(s) returned", result.row_count));

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_results() {
        let result = QueryResult {
            columns: vec!["id".to_string(), "name".to_string()],
            rows: vec![
                vec!["1".to_string(), "Alice".to_string()],
                vec!["2".to_string(), "Bob".to_string()],
                vec!["3".to_string(), "Charlie".to_string()],
            ],
            row_count: 3,
        };

        let formatted = format_results_as_table(&result);
        let expected = "id | name   \n---+--------\n1  | Alice  \n2  | Bob    \n3  | Charlie\n\n3 row(s) returned";

        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_empty_results() {
        let result = QueryResult {
            columns: vec!["id".to_string(), "name".to_string()],
            rows: Vec::new(),
            row_count: 0,
        };

        let formatted = format_results_as_table(&result);
        assert_eq!(formatted, "No results found.");
    }
}
