# 🗄️ Hyperware SQLite API Guide

## Overview

Hyperware provides SQLite database functionality through the `sqlite:distro:sys` system process. This guide covers everything from basic usage to advanced patterns for building database-driven applications using the hyperprocess framework.

## Table of Contents

1. [Prerequisites and Setup](#prerequisites-and-setup)
2. [Core SQLite API](#core-sqlite-api)
3. [Basic Usage Examples](#basic-usage-examples)
4. [Data Types and Parameters](#data-types-and-parameters)
5. [Advanced Database Patterns](#advanced-database-patterns)
6. [Error Handling](#error-handling)
7. [Performance Optimization](#performance-optimization)
8. [Complete Example Application](#complete-example-application)

## Prerequisites and Setup

### Required Capabilities

SQLite requires **TWO** capabilities in your manifest.json:

```json
{
  "request_capabilities": [
    "sqlite:distro:sys",  // For database operations
    "vfs:distro:sys"      // Required by SQLite for file storage
  ]
}
```

**Critical**: Without `vfs:distro:sys`, you'll get runtime errors like "doesn't have capability to message process vfs:distro:sys". SQLite internally uses VFS (Virtual File System) to persist database files.

### Import Statements

```rust
use hyperware_process_lib::{
    sqlite,  // SQLite functionality
    our,     // For package_id()
};
```

**Important**: Do NOT add `hyperware_process_lib` to your Cargo.toml - it's provided by the hyperprocess macro.

### State Management

```rust
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    db: Option<sqlite::Sqlite>,
    // Other state fields
}
```

## Core SQLite API

### Opening a Database

```rust
// In your #[init] function
match sqlite::open(our().package_id(), "my_database", Some(5000)) {
    Ok(db) => {
        self.db = Some(db);
        println!("Database opened successfully");
    }
    Err(e) => {
        println!("Failed to open database: {:?}", e);
    }
}
```

**Parameters:**
- `package_id`: Your app's package ID (use `our().package_id()`)
- `db_name`: Name for your database
- `timeout`: Optional timeout in milliseconds (recommended: `Some(5000)`)

### Creating Tables

```rust
if let Some(ref db) = self.db {
    let create_sql = "CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        email TEXT UNIQUE,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )".to_string();
    
    match db.write(create_sql, vec![], None) {
        Ok(_) => println!("Table created successfully"),
        Err(e) => println!("Failed to create table: {:?}", e),
    }
}
```

### Inserting Data

```rust
// Simple insert with parameters
let insert_sql = "INSERT INTO users (name, email) VALUES (?, ?)".to_string();
let params = vec![
    serde_json::Value::String("Alice".to_string()),
    serde_json::Value::String("alice@example.com".to_string()),
];

db.write(insert_sql, params, None)?;
```

### Querying Data

```rust
// Query with parameters
let query = "SELECT * FROM users WHERE email LIKE ?".to_string();
let params = vec![serde_json::Value::String("%@example.com".to_string())];

let results = db.read(query, params)?;
// results: Vec<HashMap<String, serde_json::Value>>

for row in results {
    if let Some(name) = row.get("name") {
        println!("User: {}", name);
    }
}
```

### Using Transactions

```rust
// Begin transaction
let tx_id = db.begin_tx()?;

// Execute multiple operations
let insert1 = "INSERT INTO users (name, email) VALUES (?, ?)".to_string();
let params1 = vec![
    serde_json::Value::String("Bob".to_string()),
    serde_json::Value::String("bob@example.com".to_string()),
];
db.write(insert1, params1, Some(tx_id))?;

let insert2 = "INSERT INTO users (name, email) VALUES (?, ?)".to_string();
let params2 = vec![
    serde_json::Value::String("Carol".to_string()),
    serde_json::Value::String("carol@example.com".to_string()),
];
db.write(insert2, params2, Some(tx_id))?;

// Commit transaction
db.commit_tx(tx_id)?;
```

## Basic Usage Examples

### Initialization Pattern

```rust
#[init]
async fn initialize(&mut self) {
    // Open database once at startup
    match sqlite::open(our().package_id(), "app_data", Some(5000)) {
        Ok(db) => {
            self.db = Some(db);
            
            // Create initial tables
            if let Some(ref db) = self.db {
                let sql = "CREATE TABLE IF NOT EXISTS users (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    username TEXT NOT NULL UNIQUE,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )".to_string();
                
                if let Err(e) = db.write(sql, vec![], None) {
                    println!("Failed to create table: {:?}", e);
                }
            }
        }
        Err(e) => println!("Failed to open database: {:?}", e),
    }
}
```

### HTTP Handler Example

```rust
#[http]
async fn add_user(&mut self, request_json: String) -> Result<String, String> {
    #[derive(Deserialize)]
    struct AddUserRequest {
        username: String,
    }
    
    let req: AddUserRequest = serde_json::from_str(&request_json)
        .map_err(|e| format!("Invalid request: {}", e))?;
    
    if let Some(ref db) = self.db {
        let sql = "INSERT INTO users (username) VALUES (?)".to_string();
        let params = vec![serde_json::Value::String(req.username)];
        
        db.write(sql, params, None)
            .map_err(|e| format!("Failed to insert user: {:?}", e))?;
        
        Ok("User added successfully".to_string())
    } else {
        Err("Database not connected".to_string())
    }
}
```

## Data Types and Parameters

SQLite parameters must be `serde_json::Value` variants:

```rust
// Integer
serde_json::Value::Number(serde_json::Number::from(42))

// Float
serde_json::Value::Number(serde_json::Number::from_f64(3.14).unwrap())

// String
serde_json::Value::String("Hello".to_string())

// Boolean (stored as 0/1)
serde_json::Value::Bool(true)

// Null
serde_json::Value::Null

// Using json! macro (easier)
let params = vec![
    json!("string value"),
    json!(123),
    json!(3.14),
    json!(true),
    json!(null),
];
```

### Extracting Values from Query Results

```rust
for row in results {
    // String values
    if let Some(serde_json::Value::String(name)) = row.get("name") {
        // Use name as &str
    }
    
    // Number values
    if let Some(serde_json::Value::Number(id)) = row.get("id") {
        if let Some(id_val) = id.as_i64() {
            // Use id_val as i64
        }
    }
}
```

## Advanced Database Patterns

### Multi-Table Schema with Foreign Keys

```rust
// Create related tables
let schemas = vec![
    "CREATE TABLE IF NOT EXISTS projects (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        owner_id INTEGER NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (owner_id) REFERENCES users(id)
    )",
    
    "CREATE TABLE IF NOT EXISTS tasks (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id INTEGER NOT NULL,
        assigned_to INTEGER,
        title TEXT NOT NULL,
        status TEXT DEFAULT 'pending',
        FOREIGN KEY (project_id) REFERENCES projects(id),
        FOREIGN KEY (assigned_to) REFERENCES users(id)
    )",
];

for schema in schemas {
    db.write(schema.to_string(), vec![], None)?;
}
```

### Complex Queries with JOINs

```rust
// Get project statistics with aggregations
let sql = "SELECT 
    p.id as project_id,
    p.name as project_name,
    u.name as owner_name,
    COUNT(DISTINCT t.id) as total_tasks,
    SUM(CASE WHEN t.status = 'completed' THEN 1 ELSE 0 END) as completed_tasks
FROM projects p
LEFT JOIN users u ON p.owner_id = u.id
LEFT JOIN tasks t ON p.id = t.project_id
GROUP BY p.id, p.name, u.name
ORDER BY total_tasks DESC".to_string();

let results = db.read(sql, vec![])?;
```

### Pagination Pattern

```rust
#[http]
async fn get_users_paginated(&self, request_json: String) -> Result<String, String> {
    #[derive(Deserialize)]
    struct PageRequest {
        page: u32,
        per_page: u32,
    }
    
    let req: PageRequest = serde_json::from_str(&request_json)?;
    let offset = req.page * req.per_page;
    
    if let Some(ref db) = self.db {
        let sql = "SELECT * FROM users LIMIT ? OFFSET ?".to_string();
        let params = vec![
            serde_json::Value::Number(req.per_page.into()),
            serde_json::Value::Number(offset.into()),
        ];
        
        let results = db.read(sql, params)?;
        Ok(serde_json::to_string(&results).unwrap())
    } else {
        Err("Database not connected".to_string())
    }
}
```

### Batch Operations with Transactions

```rust
#[http]
async fn batch_insert_users(&mut self, request_json: String) -> Result<String, String> {
    let users: Vec<String> = serde_json::from_str(&request_json)?;
    
    if let Some(ref db) = self.db {
        let tx_id = db.begin_tx()
            .map_err(|e| format!("Failed to begin transaction: {:?}", e))?;
        
        let mut inserted = 0;
        for username in users {
            let sql = "INSERT INTO users (username) VALUES (?)".to_string();
            let params = vec![serde_json::Value::String(username)];
            
            if db.write(sql, params, Some(tx_id)).is_ok() {
                inserted += 1;
            }
        }
        
        db.commit_tx(tx_id)
            .map_err(|e| format!("Failed to commit transaction: {:?}", e))?;
        
        Ok(format!("Inserted {} users", inserted))
    } else {
        Err("Database not connected".to_string())
    }
}
```

### Backend-Only Methods

For complex database operations that shouldn't be exposed as HTTP endpoints, use a separate `impl` block:

```rust
// HTTP handlers go in the hyperprocess impl block
#[hyperprocess(/* config */)]
impl AppState {
    #[http]
    async fn api_endpoint(&mut self, request: String) -> Result<String, String> {
        // Call internal method
        self.perform_complex_operation().await
    }
}

// Internal methods go in a separate impl block
impl AppState {
    async fn perform_complex_operation(&mut self) -> Result<String, String> {
        // Complex database logic here
    }
    
    async fn cleanup_old_records(&mut self) -> Result<usize, String> {
        if let Some(ref db) = self.db {
            let sql = "DELETE FROM logs WHERE created_at < datetime('now', '-30 days')".to_string();
            db.write(sql, vec![], None)
                .map(|_| 0) // SQLite doesn't return row count
                .map_err(|e| format!("Cleanup failed: {:?}", e))
        } else {
            Err("Database not connected".to_string())
        }
    }
}
```

## Error Handling

### Common Errors and Solutions

1. **"doesn't have capability to message process vfs:distro:sys"**
   - Add `"vfs:distro:sys"` to manifest.json capabilities

2. **"no such table: users"**
   - Ensure table creation happens before queries
   - Use `CREATE TABLE IF NOT EXISTS`

3. **"UNIQUE constraint failed"**
   ```rust
   match db.write(insert_sql, params, None) {
       Ok(_) => Ok("Inserted successfully".to_string()),
       Err(e) if e.to_string().contains("UNIQUE constraint") => {
           Err("Email already exists".to_string())
       }
       Err(e) => Err(format!("Insert failed: {:?}", e))
   }
   ```

### SQLite Error Types

- **NoDb**: Database doesn't exist
- **NoTx**: Transaction ID not found  
- **NoWriteCap**: Missing write capability
- **NoReadCap**: Missing read capability
- **NotAWriteKeyword**: Invalid SQL write statement
- **NotAReadKeyword**: Invalid SQL read statement
- **InvalidParameters**: Malformed parameter JSON
- **RusqliteError**: SQLite-specific error

## Performance Optimization

### Creating Indexes

```rust
let indexes = vec![
    "CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)",
    "CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)",
    "CREATE INDEX IF NOT EXISTS idx_tasks_assigned ON tasks(assigned_to)",
];

for index_sql in indexes {
    if let Err(e) = db.write(index_sql.to_string(), vec![], None) {
        println!("Error creating index: {:?}", e);
    }
}
```

### Database Maintenance

```rust
async fn perform_maintenance(&mut self) -> Result<String, String> {
    if let Some(ref db) = self.db {
        // Analyze tables for query optimization
        db.write("ANALYZE".to_string(), vec![], None)
            .map_err(|e| format!("Failed to analyze: {:?}", e))?;
        
        // Vacuum to reclaim space (use sparingly)
        db.write("VACUUM".to_string(), vec![], None)
            .map_err(|e| format!("Failed to vacuum: {:?}", e))?;
        
        Ok("Maintenance completed".to_string())
    } else {
        Err("Database not connected".to_string())
    }
}
```

### Best Practices

1. **Always check if database is connected** before operations
2. **Use parameterized queries** to prevent SQL injection
3. **Create indexes** for frequently queried columns
4. **Batch operations** in transactions for better performance
5. **Use `LIMIT` clauses** to control result set sizes
6. **Set appropriate timeouts** when opening databases

## Complete Example Application

```rust
use hyperprocess_macro::*;
use hyperware_process_lib::{our, sqlite, homepage::add_to_homepage};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    db: Option<sqlite::Sqlite>,
    user_count: u32,
}

#[hyperprocess(
    name = "SQLite Example",
    ui = Some(HttpBindingConfig::default()),
    endpoints = vec![
        Binding::Http { 
            path: "/api", 
            config: HttpBindingConfig::new(false, false, false, None) 
        }
    ],
    save_config = SaveOptions::EveryMessage,
    wit_world = "sqlite-example-dot-os-v0"
)]
impl AppState {
    #[init]
    async fn initialize(&mut self) {
        add_to_homepage("SQLite Example", Some("🗄️"), Some("/"), None);
        
        // Open database
        match sqlite::open(our().package_id(), "app_data", Some(5000)) {
            Ok(db) => {
                self.db = Some(db);
                
                // Create tables
                if let Some(ref db) = self.db {
                    let sql = "CREATE TABLE IF NOT EXISTS users (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        username TEXT NOT NULL UNIQUE,
                        email TEXT,
                        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                    )".to_string();
                    
                    if let Err(e) = db.write(sql, vec![], None) {
                        println!("Failed to create table: {:?}", e);
                    }
                    
                    // Create indexes
                    let index_sql = "CREATE INDEX IF NOT EXISTS idx_users_username 
                                    ON users(username)".to_string();
                    let _ = db.write(index_sql, vec![], None);
                }
            }
            Err(e) => println!("Failed to open database: {:?}", e),
        }
    }
    
    #[http]
    async fn add_user(&mut self, request_body: String) -> Result<String, String> {
        #[derive(Deserialize)]
        struct AddUserRequest {
            username: String,
            email: Option<String>,
        }
        
        let req: AddUserRequest = serde_json::from_str(&request_body)
            .map_err(|e| format!("Invalid request: {}", e))?;
        
        if let Some(ref db) = self.db {
            let sql = "INSERT INTO users (username, email) VALUES (?, ?)".to_string();
            let params = vec![
                json!(req.username),
                req.email.map(|e| json!(e)).unwrap_or(json!(null)),
            ];
            
            match db.write(sql, params, None) {
                Ok(_) => {
                    self.user_count += 1;
                    Ok(json!({
                        "success": true,
                        "message": "User added successfully"
                    }).to_string())
                }
                Err(e) if e.to_string().contains("UNIQUE") => {
                    Err("Username already exists".to_string())
                }
                Err(e) => Err(format!("Failed to insert user: {:?}", e))
            }
        } else {
            Err("Database not connected".to_string())
        }
    }
    
    #[http]
    async fn get_users(&self, request_body: String) -> Result<String, String> {
        #[derive(Deserialize)]
        struct GetUsersRequest {
            page: Option<u32>,
            per_page: Option<u32>,
        }
        
        let req: GetUsersRequest = serde_json::from_str(&request_body)
            .unwrap_or(GetUsersRequest { page: None, per_page: None });
        
        if let Some(ref db) = self.db {
            let (sql, params) = if let (Some(page), Some(per_page)) = (req.page, req.per_page) {
                let offset = page * per_page;
                (
                    "SELECT * FROM users ORDER BY created_at DESC LIMIT ? OFFSET ?".to_string(),
                    vec![json!(per_page), json!(offset)]
                )
            } else {
                (
                    "SELECT * FROM users ORDER BY created_at DESC".to_string(),
                    vec![]
                )
            };
            
            let results = db.read(sql, params)
                .map_err(|e| format!("Query failed: {:?}", e))?;
            
            Ok(json!({
                "users": results,
                "total_count": self.user_count
            }).to_string())
        } else {
            Err("Database not connected".to_string())
        }
    }
    
    #[http]
    async fn search_users(&self, request_body: String) -> Result<String, String> {
        #[derive(Deserialize)]
        struct SearchRequest {
            query: String,
        }
        
        let req: SearchRequest = serde_json::from_str(&request_body)
            .map_err(|e| format!("Invalid request: {}", e))?;
        
        if let Some(ref db) = self.db {
            let sql = "SELECT * FROM users 
                      WHERE username LIKE ? OR email LIKE ?
                      ORDER BY username".to_string();
            let pattern = format!("%{}%", req.query);
            let params = vec![json!(pattern.clone()), json!(pattern)];
            
            let results = db.read(sql, params)
                .map_err(|e| format!("Search failed: {:?}", e))?;
            
            Ok(json!({
                "results": results,
                "count": results.len()
            }).to_string())
        } else {
            Err("Database not connected".to_string())
        }
    }
}

// Internal methods in separate impl block
impl AppState {
    async fn get_user_by_id(&self, user_id: i64) -> Result<Option<HashMap<String, serde_json::Value>>, String> {
        if let Some(ref db) = self.db {
            let sql = "SELECT * FROM users WHERE id = ?".to_string();
            let params = vec![json!(user_id)];
            
            let mut results = db.read(sql, params)
                .map_err(|e| format!("Query failed: {:?}", e))?;
            
            Ok(results.pop())
        } else {
            Err("Database not connected".to_string())
        }
    }
}
```

## Testing Your Implementation

1. **Build**: `kit b` or `kit b --hyperapp`
2. **Start**: `kit s`
3. **Install**: Install your app through the Hyperware UI
4. **Test API calls**:
   ```bash
   # Add user
   curl -X POST http://localhost:8080/your-app:your-publisher/api \
     -H "Content-Type: application/json" \
     -d '{"AddUser": {"username": "alice", "email": "alice@example.com"}}'
   
   # Get users
   curl -X POST http://localhost:8080/your-app:your-publisher/api \
     -H "Content-Type: application/json" \
     -d '{"GetUsers": {"page": 0, "per_page": 10}}'
   ```

## Summary

The SQLite API in Hyperware provides a complete database solution with:
- Persistent storage with full SQL support
- Safe parameterized queries to prevent SQL injection
- Transaction support with automatic rollback
- JSON-compatible result format
- Support for complex queries, JOINs, and aggregations

Remember the key requirements:
- Include both `sqlite:distro:sys` and `vfs:distro:sys` capabilities
- Open the database once in your `#[init]` function
- Always check if the database is connected before operations
- Use parameterized queries for all user input
- Separate HTTP handlers from internal database methods