use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::debug;

/// Represents a database connection
#[derive(Debug)]
pub struct DbConnection {
    /// Path to the database file
    path: PathBuf,
    /// Connection to the database
    connection: Arc<Mutex<Connection>>,
}

/// Type of database connection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionType {
    /// In-memory database
    Memory,
    /// File-based database
    File,
}

impl DbConnection {
    /// Open a new database connection
    pub fn open(path: Option<impl AsRef<Path>>) -> Result<Self> {
        match path {
            Some(path) => {
                let path_buf = PathBuf::from(path.as_ref());
                debug!("Opening database connection to {}", path_buf.display());

                let conn =
                    Connection::open(&path_buf).context("Failed to open database connection")?;

                Ok(Self {
                    path: path_buf,
                    connection: Arc::new(Mutex::new(conn)),
                })
            }
            None => {
                debug!("Opening in-memory database connection");

                let conn = Connection::open_in_memory()
                    .context("Failed to open in-memory database connection")?;

                Ok(Self {
                    path: PathBuf::from(":memory:"),
                    connection: Arc::new(Mutex::new(conn)),
                })
            }
        }
    }

    /// Get the connection type
    pub fn connection_type(&self) -> ConnectionType {
        if self.path.to_string_lossy() == ":memory:" {
            ConnectionType::Memory
        } else {
            ConnectionType::File
        }
    }

    /// Execute a SQL statement
    pub fn execute(&self, sql: &str) -> Result<usize> {
        debug!("Executing SQL: {}", sql);

        let conn = self.connection.lock().unwrap();
        conn.execute(sql, [])
            .with_context(|| format!("Failed to execute SQL: {}", sql))
    }

    /// Execute a query and return the results as strings
    pub fn query(&self, sql: &str) -> Result<Vec<Vec<String>>> {
        debug!("Executing query: {}", sql);

        let conn = self.connection.lock().unwrap();
        let mut stmt = conn
            .prepare(sql)
            .with_context(|| format!("Failed to prepare SQL: {}", sql))?;

        let column_count = stmt.column_count();
        let mut rows = stmt
            .query([])
            .with_context(|| format!("Failed to execute query: {}", sql))?;

        let mut results = Vec::new();

        while let Some(row) = rows.next()? {
            let mut row_data = Vec::with_capacity(column_count);

            for i in 0..column_count {
                let value = match row.get_ref(i)? {
                    rusqlite::types::ValueRef::Null => "NULL".to_string(),
                    rusqlite::types::ValueRef::Integer(i) => i.to_string(),
                    rusqlite::types::ValueRef::Real(f) => f.to_string(),
                    rusqlite::types::ValueRef::Text(s) => String::from_utf8_lossy(s).to_string(),
                    rusqlite::types::ValueRef::Blob(b) => format!("<BLOB: {} bytes>", b.len()),
                };

                row_data.push(value);
            }

            results.push(row_data);
        }

        Ok(results)
    }

    /// Close the database connection
    pub fn close(self) -> Result<()> {
        debug!("Closing database connection to {}", self.path.display());

        // The connection will be closed when it's dropped
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_in_memory_connection() {
        let conn = DbConnection::open(None::<&str>).unwrap();
        assert_eq!(conn.connection_type(), ConnectionType::Memory);

        // Create a test table
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)")
            .unwrap();
        conn.execute("INSERT INTO test (id, name) VALUES (1, 'test')")
            .unwrap();

        // Query the table
        let results = conn.query("SELECT * FROM test").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0], "1");
        assert_eq!(results[0][1], "test");
    }

    #[test]
    fn test_file_connection() {
        let file = NamedTempFile::new().unwrap();
        let conn = DbConnection::open(Some(file.path())).unwrap();
        assert_eq!(conn.connection_type(), ConnectionType::File);

        // Create a test table
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)")
            .unwrap();
        conn.execute("INSERT INTO test (id, name) VALUES (1, 'test')")
            .unwrap();

        // Query the table
        let results = conn.query("SELECT * FROM test").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0], "1");
        assert_eq!(results[0][1], "test");
    }
}
