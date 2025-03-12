# VFS API

## Drives

A drive is a directory within a package's VFS directory, e.g., `app-store:sys/pkg/` or `your_package:publisher.os/my_drive/`.
Drives are owned by packages.
Packages can share access to drives they own via [capabilities](../system/process/capabilities.md).
Each package is spawned with two drives: [`pkg/`](#pkg-drive) and [`tmp/`](#tmp-drive).
All processes in a package have caps to those drives.
Processes can also create additional drives.

### `pkg/` drive

The `pkg/` drive contains metadata about the package that Hyperware requires to run that package, `.wasm` binaries, and optionally the API of the package and the UI.
When creating packages, the `pkg/` drive is populated by [`kit build`](../kit/build.md) and loaded into the Hyperware node using [`kit start-package`](../kit/start-package.md).

### `tmp/` drive

The `tmp/` drive can be written to directly by the owning package using standard filesystem functionality (i.e. `std::fs` in Rust) via WASI in addition to the Hyperware VFS.

### Imports

```rust
use hyperware_process_lib::vfs::{
  create_drive, open_file, open_dir, create_file, metadata, File, Directory,
};
```

### Opening/Creating a Drive

```rust
let drive_path: String = create_drive(our.package_id(), "drive_name")?;
// you can now prepend this path to any files/directories you're interacting with
let file = open_file(&format!("{}/hello.txt", &drive_path), true);
```

### Sharing a Drive Capability

```rust
let vfs_read_cap = serde_json::json!({
    "kind": "read",
    "drive": drive_path,
}).to_string();

let vfs_address = Address {
    node: our.node.clone(),
    process: ProcessId::from_str("vfs:distro:sys").unwrap(),
};

// get this capability from our store
let cap = get_capability(&vfs_address, &vfs_read_cap);

// now if we have that Capability, we can attach it to a subsequent message.
if let Some(cap) = cap {
    Request::new()
        .capabilities(vec![cap])
        .body(b"hello".to_vec())
        .send()?;
}
```

```rust
// the receiving process can then save the capability to it's store, and open the drive.
save_capabilities(incoming_request.capabilities);
let dir = open_dir(&drive_path, false)?;
```

### Files

#### Open a File

```rust
/// Opens a file at path, if no file at path, creates one if boolean create is true.
let file_path = format!("{}/hello.txt", &drive_path);
let file = open_file(&file_path, true);
```

#### Create a File

```rust
/// Creates a file at path, if file found at path, truncates it to 0.
let file_path = format!("{}/hello.txt", &drive_path);
let file = create_file(&file_path);
```

#### Read a File

```rust
/// Reads the entire file, from start position.
/// Returns a vector of bytes.
let contents = file.read()?;
```

#### Write a File

```rust
/// Write entire slice as the new file.
/// Truncates anything that existed at path before.
let buffer = b"Hello!";
file.write(&buffer)?;
```

#### Write to File

```rust
/// Write buffer to file at current position, overwriting any existing data.
let buffer = b"World!";
file.write_all(&buffer)?;
```

#### Read at position

```rust
/// Read into buffer from current cursor position
/// Returns the amount of bytes read.
let mut buffer = vec![0; 5];
file.read_at(&buffer)?;
```

#### Set Length

```rust
/// Set file length, if given size > underlying file, fills it with 0s.
file.set_len(42)?;
```

#### Seek to a position

```rust
/// Seek file to position.
/// Returns the new position.
let position = SeekFrom::End(0);
file.seek(&position)?;
```

#### Sync

```rust
/// Syncs path file buffers to disk.
file.sync_all()?;
```

#### Metadata

```rust
/// Metadata of a path, returns file type and length.
let metadata = file.metadata()?;
```

### Directories

#### Open a Directory

```rust
/// Opens or creates a directory at path.
/// If trying to create an existing file, will just give you the path.
let dir_path = format!("{}/my_pics", &drive_path);
let dir = open_dir(&dir_path, true);
```

#### Read a Directory

```rust
/// Iterates through children of directory, returning a vector of DirEntries.
/// DirEntries contain the path and file type of each child.
let entries = dir.read()?;
```

#### General path Metadata

```rust
/// Metadata of a path, returns file type and length.
let some_path = format!("{}/test", &drive_path);
let metadata = metadata(&some_path)?;
```

### API

```rust
/// IPC Request format for the vfs:distro:sys runtime module.
#[derive(Debug, Serialize, Deserialize)]
pub struct VfsRequest {
    pub path: String,
    pub action: VfsAction,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum VfsAction {
    CreateDrive,
    CreateDir,
    CreateDirAll,
    CreateFile,
    OpenFile { create: bool },
    CloseFile,
    Write,
    WriteAll,
    Append,
    SyncAll,
    Read,
    ReadDir,
    ReadToEnd,
    ReadExact { length: u64 },
    ReadToString,
    Seek(SeekFrom),
    RemoveFile,
    RemoveDir,
    RemoveDirAll,
    Rename { new_path: String },
    Metadata,
    AddZip,
    CopyFile { new_path: String },
    Len,
    SetLen(u64),
    Hash,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FileType {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMetadata {
    pub file_type: FileType,
    pub len: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirEntry {
    pub path: String,
    pub file_type: FileType,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum VfsResponse {
    Ok,
    Err(VfsError),
    Read,
    SeekFrom { new_offset: u64 },
    ReadDir(Vec<DirEntry>),
    ReadToString(String),
    Metadata(FileMetadata),
    Len(u64),
    Hash([u8; 32]),
}

#[derive(Error, Debug, Serialize, Deserialize)]
pub enum VfsError {
    #[error("No capability for action {action} at path {path}")]
    NoCap { action: String, path: String },
    #[error("Bytes blob required for {action} at path {path}")]
    BadBytes { action: String, path: String },
    #[error("bad request error: {error}")]
    BadRequest { error: String },
    #[error("error parsing path: {path}: {error}")]
    ParseError { error: String, path: String },
    #[error("IO error: {error}, at path {path}")]
    IOError { error: String, path: String },
    #[error("kernel capability channel error: {error}")]
    CapChannelFail { error: String },
    #[error("Bad JSON blob: {error}")]
    BadJson { error: String },
    #[error("File not found at path {path}")]
    NotFound { path: String },
    #[error("Creating directory failed at path: {path}: {error}")]
    CreateDirError { path: String, error: String },
    #[error("Other error: {error}")]
    Other { error: String },
}
```