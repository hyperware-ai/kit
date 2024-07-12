use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use color_eyre::{
    Section,
    {
        eyre::{eyre, WrapErr},
        Result,
    },
};
use fs_err as fs;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument, warn};

use kinode_process_lib::{PackageId, kernel_types::Erc721Metadata};

use crate::setup::{
    check_js_deps, check_py_deps, check_rust_deps, get_deps, get_newest_valid_node_version,
    get_python_version, REQUIRED_PY_PACKAGE,
};
use crate::start_package::zip_directory;
use crate::view_api;
use crate::KIT_CACHE;

const PY_VENV_NAME: &str = "process_env";
const JAVASCRIPT_SRC_PATH: &str = "src/lib.js";
const PYTHON_SRC_PATH: &str = "src/lib.py";
const RUST_SRC_PATH: &str = "src/lib.rs";
const KINODE_WIT_0_7_0_URL: &str =
    "https://raw.githubusercontent.com/kinode-dao/kinode-wit/aa2c8b11c9171b949d1991c32f58591c0e881f85/kinode.wit";
const KINODE_WIT_0_8_0_URL: &str =
    "https://raw.githubusercontent.com/kinode-dao/kinode-wit/v0.8/kinode.wit";
const WASI_VERSION: &str = "19.0.1"; // TODO: un-hardcode
const DEFAULT_WORLD_0_7_0: &str = "process";
const DEFAULT_WORLD_0_8_0: &str = "process-v0";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CargoFile {
    package: CargoPackage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CargoPackage {
    name: String,
}

#[instrument(level = "trace", skip_all)]
pub fn has_feature(cargo_toml_path: &str, feature: &str) -> Result<bool> {
    let cargo_toml_content = fs::read_to_string(cargo_toml_path)?;
    let cargo_toml: toml::Value = cargo_toml_content.parse()?;

    if let Some(features) = cargo_toml.get("features").and_then(|f| f.as_table()) {
        Ok(features.contains_key(feature))
    } else {
        Ok(false)
    }
}

#[instrument(level = "trace", skip_all)]
pub fn remove_missing_features(
    cargo_toml_path: &Path,
    features: Vec<&str>,
) -> Result<Vec<String>> {
    let cargo_toml_content = fs::read_to_string(cargo_toml_path)?;
    let cargo_toml: toml::Value = cargo_toml_content.parse()?;
    let Some(cargo_features) = cargo_toml.get("features").and_then(|f| f.as_table()) else {
        return Ok(vec![]);
    };

    Ok(features
        .iter()
        .filter_map(|f| {
            let f = f.to_string();
            if cargo_features.contains_key(&f) {
                Some(f)
            } else {
                None
            }
        })
        .collect()
    )
}

/// Check if the first element is empty and there are no more elements
#[instrument(level = "trace", skip_all)]
fn is_only_empty_string(splitted: &Vec<&str>) -> bool {
    let mut parts = splitted.iter();
    parts.next() == Some(&"") && parts.next().is_none()
}

#[instrument(level = "trace", skip_all)]
pub fn run_command(cmd: &mut Command, verbose: bool) -> Result<Option<(String, String)>> {
    if verbose {
        let mut child = cmd.spawn()?;
        child.wait()?;
        return Ok(None);
    }
    let output = cmd.output()?;
    if output.status.success() {
        Ok(Some((
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )))
    } else {
        Err(eyre!(
            "Command `{} {:?}` failed with exit code {:?}\nstdout: {}\nstderr: {}",
            cmd.get_program().to_str().unwrap(),
            cmd.get_args()
                .map(|a| a.to_str().unwrap())
                .collect::<Vec<_>>(),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ))
    }
}

#[instrument(level = "trace", skip_all)]
pub async fn download_file(url: &str, path: &Path) -> Result<()> {
    fs::create_dir_all(&KIT_CACHE)?;
    let hex_url = hex::encode(url);
    let hex_url_path = format!("{}/{}", KIT_CACHE, hex_url);
    let hex_url_path = Path::new(&hex_url_path);

    let content = if hex_url_path.exists() {
        fs::read(hex_url_path)?
    } else {
        let response = reqwest::get(url).await?;

        // Check if response status is 200 (OK)
        if response.status() != reqwest::StatusCode::OK {
            return Err(eyre!(
                "Failed to download file: HTTP Status {}",
                response.status()
            ));
        }

        let content = response.bytes().await?.to_vec();
        fs::write(hex_url_path, &content)?;
        content
    };

    if path.exists() {
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            let existing_content = fs::read(path)?;
            if content == existing_content {
                return Ok(());
            }
        }
    }
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| eyre!("path doesn't have parent"))?,
    )?;
    fs::write(path, &content)?;
    Ok(())
}

#[instrument(level = "trace", skip_all)]
pub fn read_metadata(package_dir: &Path) -> Result<Erc721Metadata> {
    let metadata: Erc721Metadata =
        serde_json::from_reader(fs::File::open(package_dir.join("metadata.json"))
            .wrap_err_with(|| "Missing required metadata.json file. See discussion at https://book.kinode.org/my_first_app/chapter_1.html?highlight=metadata.json#metadatajson")?
        )?;
    Ok(metadata)
}

/// Regex to dynamically capture the world name after 'world'
fn extract_world(data: &str) -> Option<String> {
    let re = regex::Regex::new(r"world\s+([^\s\{]+)").unwrap();
    re.captures(data)
        .and_then(|caps| caps.get(1).map(|match_| match_.as_str().to_string()))
}

fn extract_worlds_from_files(directory: &Path) -> Vec<String> {
    let mut worlds = vec![];

    // Safe to return early if directory reading fails
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => return worlds,
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file()
            || Some("kinode.wit") == path.file_name().and_then(|s| s.to_str())
            || Some("wit") != path.extension().and_then(|s| s.to_str())
        {
            continue;
        }
        let contents = fs::read_to_string(&path).unwrap_or_default();
        if let Some(world) = extract_world(&contents) {
            worlds.push(world);
        }
    }

    worlds
}

fn get_world_or_default(directory: &Path, default_world: String) -> String {
    let worlds = extract_worlds_from_files(directory);
    if worlds.len() == 1 {
        return worlds[0].clone();
    }
    warn!(
        "Found {} worlds in {directory:?}; defaulting to {default_world}",
        worlds.len()
    );
    default_world
}

#[instrument(level = "trace", skip_all)]
fn copy_dir(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[instrument(level = "trace", skip_all)]
async fn compile_javascript_wasm_process(
    process_dir: &Path,
    valid_node: Option<String>,
    world: String,
    verbose: bool,
) -> Result<()> {
    info!(
        "Compiling Javascript Kinode process in {:?}...",
        process_dir
    );

    let wasm_file_name = process_dir.file_name().and_then(|s| s.to_str()).unwrap();
    let world_name = get_world_or_default(&process_dir.join("target").join("wit"), world);

    let install = "npm install".to_string();
    let componentize = format!("node componentize.mjs {wasm_file_name} {world_name}");
    let (install, componentize) = valid_node
        .map(|valid_node| {
            (
                format!(
                    "source ~/.nvm/nvm.sh && nvm use {} && {}",
                    valid_node, install
                ),
                format!(
                    "source ~/.nvm/nvm.sh && nvm use {} && {}",
                    valid_node, componentize
                ),
            )
        })
        .unwrap_or_else(|| (install, componentize));

    run_command(
        Command::new("bash")
            .args(&["-c", &install])
            .current_dir(process_dir),
        verbose,
    )?;

    run_command(
        Command::new("bash")
            .args(&["-c", &componentize])
            .current_dir(process_dir),
        verbose,
    )?;

    info!(
        "Done compiling Javascript Kinode process in {:?}.",
        process_dir
    );
    Ok(())
}

#[instrument(level = "trace", skip_all)]
async fn compile_python_wasm_process(
    process_dir: &Path,
    python: &str,
    world: String,
    verbose: bool,
) -> Result<()> {
    info!("Compiling Python Kinode process in {:?}...", process_dir);

    let wasm_file_name = process_dir.file_name().and_then(|s| s.to_str()).unwrap();
    let world_name = get_world_or_default(&process_dir.join("target").join("wit"), world);

    let source = format!("source ../{PY_VENV_NAME}/bin/activate");
    let install = format!("pip install {REQUIRED_PY_PACKAGE}");
    let componentize = format!(
        "componentize-py -d ../target/wit/ -w {} componentize lib -o ../../pkg/{}.wasm",
        world_name, wasm_file_name,
    );

    run_command(
        Command::new(python)
            .args(&["-m", "venv", PY_VENV_NAME])
            .current_dir(process_dir),
        verbose,
    )?;
    run_command(
        Command::new("bash")
            .args(&["-c", &format!("{source} && {install} && {componentize}")])
            .current_dir(process_dir.join("src")),
        verbose,
    )?;

    info!("Done compiling Python Kinode process in {:?}.", process_dir);
    Ok(())
}

#[instrument(level = "trace", skip_all)]
async fn compile_rust_wasm_process(
    process_dir: &Path,
    features: &str,
    verbose: bool,
) -> Result<()> {
    info!("Compiling Rust Kinode process in {:?}...", process_dir);

    // Paths
    let wit_dir = process_dir.join("target").join("wit");
    let bindings_dir = process_dir
        .join("target")
        .join("bindings")
        .join(process_dir.file_name().unwrap());
    fs::create_dir_all(&bindings_dir)?;

    // Check and download wasi_snapshot_preview1.wasm if it does not exist
    let wasi_snapshot_file = process_dir
        .join("target")
        .join("wasi_snapshot_preview1.wasm");
    let wasi_snapshot_url = format!(
        "https://github.com/bytecodealliance/wasmtime/releases/download/v{}/wasi_snapshot_preview1.reactor.wasm",
        WASI_VERSION,
    );
    download_file(&wasi_snapshot_url, &wasi_snapshot_file).await?;

    // Copy wit directory to bindings
    fs::create_dir_all(&bindings_dir.join("wit"))?;
    for entry in fs::read_dir(&wit_dir)? {
        let entry = entry?;
        fs::copy(
            entry.path(),
            bindings_dir.join("wit").join(entry.file_name()),
        )?;
    }

    // Build the module using Cargo
    let mut args = vec![
        "+nightly",
        "build",
        "--release",
        "--no-default-features",
        "--target",
        "wasm32-wasi",
        "--target-dir",
        "target",
        "--color=always",
    ];
    let test_only = features == "test";
    let features: Vec<&str> = features.split(',').collect();
    let original_length = if is_only_empty_string(&features) {
        0
    } else {
        features.len()
    };
    let features = remove_missing_features(
        &process_dir.join("Cargo.toml"),
        features,
    )?;
    if !test_only && original_length != features.len() {
        info!("process {:?} missing features; using {:?}", process_dir, features);
    };
    let features = features.join(",");
    if !features.is_empty() {
        args.push("--features");
        args.push(&features);
    }
    let result = run_command(
        Command::new("cargo").args(&args).current_dir(process_dir),
        verbose,
    )?;

    if let Some((stdout, stderr)) = result {
        if stdout.contains("warning") {
            warn!("{}", stdout);
        }
        if stderr.contains("warning") {
            warn!("{}", stderr);
        }
    }

    // Adapt the module using wasm-tools

    // For use inside of process_dir
    let wasm_file_name = process_dir.file_name().and_then(|s| s.to_str()).unwrap();

    let wasm_file_prefix = Path::new("target/wasm32-wasi/release");
    let wasm_file = wasm_file_prefix.join(&format!("{}.wasm", wasm_file_name));

    let wasm_path = format!("../pkg/{}.wasm", wasm_file_name);
    let wasm_path = Path::new(&wasm_path);

    let wasi_snapshot_file = Path::new("target/wasi_snapshot_preview1.wasm");

    run_command(
        Command::new("wasm-tools")
            .args(&[
                "component",
                "new",
                wasm_file.to_str().unwrap(),
                "-o",
                wasm_path.to_str().unwrap(),
                "--adapt",
                wasi_snapshot_file.to_str().unwrap(),
            ])
            .current_dir(process_dir),
        verbose,
    )?;

    info!("Done compiling Rust Kinode process in {:?}.", process_dir);
    Ok(())
}

#[instrument(level = "trace", skip_all)]
async fn compile_and_copy_ui(
    package_dir: &Path,
    valid_node: Option<String>,
    verbose: bool,
) -> Result<()> {
    let ui_path = package_dir.join("ui");
    info!("Building UI in {:?}...", ui_path);

    if ui_path.exists() && ui_path.is_dir() {
        if ui_path.join("package.json").exists() {
            info!("UI directory found, running npm install...");

            let install = "npm install".to_string();
            let run = "npm run build:copy".to_string();
            let (install, run) = valid_node
                .map(|valid_node| {
                    (
                        format!(
                            "source ~/.nvm/nvm.sh && nvm use {} && {}",
                            valid_node, install
                        ),
                        format!("source ~/.nvm/nvm.sh && nvm use {} && {}", valid_node, run),
                    )
                })
                .unwrap_or_else(|| (install, run));

            run_command(
                Command::new("bash")
                    .args(&["-c", &install])
                    .current_dir(&ui_path),
                verbose,
            )?;

            info!("Running npm run build:copy...");

            run_command(
                Command::new("bash")
                    .args(&["-c", &run])
                    .current_dir(&ui_path),
                verbose,
            )?;
        } else {
            let pkg_ui_path = package_dir.join("pkg/ui");
            if pkg_ui_path.exists() {
                fs::remove_dir_all(&pkg_ui_path)?;
            }
            run_command(
                Command::new("cp")
                    .args(["-r", "ui", "pkg/ui"])
                    .current_dir(&package_dir),
                verbose,
            )?;
        }
    } else {
        return Err(eyre!("'ui' directory not found"));
    }

    info!("Done building UI in {:?}.", ui_path);
    Ok(())
}

#[instrument(level = "trace", skip_all)]
async fn compile_package_and_ui(
    package_dir: &Path,
    valid_node: Option<String>,
    skip_deps_check: bool,
    features: &str,
    url: Option<String>,
    default_world: Option<String>,
    download_from: Option<&str>,
    verbose: bool,
) -> Result<()> {
    compile_and_copy_ui(package_dir, valid_node, verbose).await?;
    compile_package(
        package_dir,
        skip_deps_check,
        features,
        url,
        default_world,
        download_from,
        verbose,
    )
    .await?;
    Ok(())
}

#[instrument(level = "trace", skip_all)]
async fn build_wit_dir(
    process_dir: &Path,
    apis: &HashMap<String, Vec<u8>>,
    wit_version: Option<u32>,
) -> Result<()> {
    let wit_dir = process_dir.join("target").join("wit");
    let wit_url = match wit_version {
        None => KINODE_WIT_0_7_0_URL,
        Some(0) | _ => KINODE_WIT_0_8_0_URL,
    };
    download_file(wit_url, &wit_dir.join("kinode.wit")).await?;
    for (file_name, contents) in apis {
        fs::write(wit_dir.join(file_name), contents)?;
    }
    Ok(())
}

#[instrument(level = "trace", skip_all)]
async fn compile_package_item(
    entry: std::io::Result<std::fs::DirEntry>,
    features: String,
    apis: HashMap<String, Vec<u8>>,
    world: String,
    wit_version: Option<u32>,
    verbose: bool,
) -> Result<()> {
    let entry = entry?;
    let path = entry.path();
    if path.is_dir() {
        let is_rust_process = path.join(RUST_SRC_PATH).exists();
        let is_py_process = path.join(PYTHON_SRC_PATH).exists();
        let is_js_process = path.join(JAVASCRIPT_SRC_PATH).exists();
        if is_rust_process || is_py_process || is_js_process {
            build_wit_dir(&path, &apis, wit_version).await?;
        }

        if is_rust_process {
            compile_rust_wasm_process(&path, &features, verbose).await?;
        } else if is_py_process {
            let python = get_python_version(None, None)?
                .ok_or_else(|| eyre!("kit requires Python 3.10 or newer"))?;
            compile_python_wasm_process(&path, &python, world, verbose).await?;
        } else if is_js_process {
            let valid_node = get_newest_valid_node_version(None, None)?;
            compile_javascript_wasm_process(&path, valid_node, world, verbose).await?;
        }
    }
    Ok(())
}

#[instrument(level = "trace", skip_all)]
async fn fetch_dependencies(
    dependencies: &Vec<String>,
    apis: &mut HashMap<String, Vec<u8>>,
    wasm_paths: &mut HashSet<PathBuf>,
    url: String,
    download_from: Option<&str>,
) -> Result<()> {
    for dependency in dependencies {
        if dependency.parse::<PackageId>().is_err() {
            return Err(eyre!(
                "Dependencies must be PackageIds (e.g. `package:publisher.os`); given {dependency}.",
            ));
        };
        let Some(zip_dir) = view_api::execute(
            None,
            Some(dependency),
            &url,
            download_from,
            false,
        ).await? else {
            return Err(eyre!(
                "Got unexpected result from fetching API for {dependency}"
            ));
        };
        for entry in zip_dir.read_dir()? {
            let entry = entry?;
            let path = entry.path();
            let maybe_ext = path.extension().and_then(|s| s.to_str());
            if Some("wit") == maybe_ext {
                let file_name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                let wit_contents = fs::read(&path)?;
                apis.insert(file_name.into(), wit_contents);
            } else if Some("wasm") == maybe_ext {
                wasm_paths.insert(path);
            }
        }
    }
    Ok(())
}

fn extract_imports_exports_from_wit(input: &str) -> (Vec<String>, Vec<String>) {
    let import_re = regex::Regex::new(r"import\s+([^\s;]+)").unwrap();
    let export_re = regex::Regex::new(r"export\s+([^\s;]+)").unwrap();
    let imports: Vec<String> = import_re.captures_iter(input)
        .map(|cap| cap[1].to_string())
        .filter(|s| !(s.contains("wasi") || s.contains("kinode:process/standard")))
        .collect();

    let exports: Vec<String> = export_re.captures_iter(input)
        .map(|cap| cap[1].to_string())
        .filter(|s| !s.contains("init"))
        .collect();

    (imports, exports)
}

#[instrument(level = "trace", skip_all)]
fn get_imports_exports_from_wasm(
    path: &PathBuf,
    imports: &mut HashMap<String, Vec<PathBuf>>,
    exports: &mut HashMap<String, PathBuf>,
    should_move_export: bool,
) -> Result<()> {
    let wit = run_command(
        Command::new("wasm-tools")
            .args(["component", "wit", path.to_str().unwrap()]),
        false,
    )?;
    let Some((ref wit, _)) = wit else {
        return Ok(());
    };
    let (wit_imports, wit_exports) = extract_imports_exports_from_wit(wit);
    for wit_import in wit_imports {
        imports
            .entry(wit_import)
            .or_insert_with(Vec::new)
            .push(path.clone());
    }
    for wit_export in wit_exports {
        if exports.contains_key(&wit_export) {
            return Err(eyre!(
                "found multiple exporters of {wit_export}: {path:?} & {exports:?}",
            ));
        }
        let path = if should_move_export {
            let file_name = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap()
                .replace("_", "-");
            let new_path = path
                .parent()
                .and_then(|p| p.parent())
                .unwrap()
                .join("target")
                .join("api")
                .join(file_name);
            fs::rename(&path, &new_path)?;
            new_path
        } else {
            path.clone()
        };

        exports.insert(wit_export, path);
    }
    Ok(())
}

#[instrument(level = "trace", skip_all)]
fn find_non_standard(
    package_dir: &Path,
    wasm_paths: HashSet<PathBuf>,
) -> Result<(HashMap<String, Vec<PathBuf>>, HashMap<String, PathBuf>)> {
    let mut imports = HashMap::new();
    let mut exports = HashMap::new();

    for entry in package_dir.join("pkg").read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if !(path.is_file() && Some("wasm") == path.extension().and_then(|e| e.to_str())) {
            continue;
        }
        get_imports_exports_from_wasm(&path, &mut imports, &mut exports, true)?;
    }
    for wasm_path in wasm_paths {
        get_imports_exports_from_wasm(&wasm_path, &mut imports, &mut exports, false)?;
    }

    Ok((imports, exports))
}

/// package dir looks like:
/// ```
/// metadata.json
/// api/                                  <- optional
///   my_package:publisher.os-v0.wit
/// pkg/
///   api.zip                             <- built
///   manifest.json
///   process_i.wasm                      <- built
///   projess_j.wasm                      <- built
/// process_i/
///   src/
///     lib.rs
///   target/                             <- built
///     api/
///     wit/
/// process_j/
///   src/
///   target/                             <- built
///     api/
///     wit/
/// ```
#[instrument(level = "trace", skip_all)]
async fn compile_package(
    package_dir: &Path,
    skip_deps_check: bool,
    features: &str,
    url: Option<String>,
    default_world: Option<String>,
    download_from: Option<&str>,
    verbose: bool,
) -> Result<()> {
    let metadata = read_metadata(package_dir)?;
    let mut checked_rust = false;
    let mut checked_py = false;
    let mut checked_js = false;
    let mut apis = HashMap::new();
    let mut wasm_paths = HashSet::new();
    for entry in package_dir.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path.join(RUST_SRC_PATH).exists() && !checked_rust && !skip_deps_check {
                let deps = check_rust_deps()?;
                get_deps(deps, verbose)?;
                checked_rust = true;
            } else if path.join(PYTHON_SRC_PATH).exists() && !checked_py {
                check_py_deps()?;
                checked_py = true;
            } else if path.join(JAVASCRIPT_SRC_PATH).exists() && !checked_js && !skip_deps_check {
                let deps = check_js_deps()?;
                get_deps(deps, verbose)?;
                checked_js = true;
            } else if Some("api") == path.file_name().and_then(|s| s.to_str()) {
                // read api files: to be used in build
                for entry in path.read_dir()? {
                    let entry = entry?;
                    let path = entry.path();
                    if Some("wit") != path.extension().and_then(|e| e.to_str()) {
                        continue;
                    };
                    let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
                        continue;
                    };
                    // TODO: reenable check once deps are working
                    // if file_name.starts_with(&format!(
                    //     "{}:{}",
                    //     metadata.properties.package_name,
                    //     metadata.properties.publisher,
                    // )) {
                    //     if let Ok(api_contents) = fs::read(&path) {
                    //         apis.insert(file_name.to_string(), api_contents);
                    //     }
                    // }
                    if let Ok(api_contents) = fs::read(&path) {
                        apis.insert(file_name.to_string(), api_contents);
                    }
                }

                // fetch dependency apis: to be used in build
                if let Some(ref dependencies) = metadata.properties.dependencies {
                    if dependencies.is_empty() {
                        continue;
                    }
                    let Some(ref url) = url else {
                        // TODO: can we use kit-cached deps?
                        return Err(eyre!("Need a node to be able to fetch dependencies"));
                    };
                    fetch_dependencies(
                        dependencies,
                        &mut apis,
                        &mut wasm_paths,
                        url.clone(),
                        download_from,
                    ).await?;
                }
            }
        }
    }

    let wit_world = default_world.unwrap_or_else(|| match metadata.properties.wit_version {
        None => DEFAULT_WORLD_0_7_0.to_string(),
        Some(0) | _ => DEFAULT_WORLD_0_8_0.to_string(),
    });

    let mut tasks = tokio::task::JoinSet::new();
    let features = features.to_string();
    for entry in package_dir.read_dir()? {
        tasks.spawn(compile_package_item(
            entry,
            features.clone(),
            apis.clone(),
            wit_world.clone(),
            metadata.properties.wit_version,
            verbose.clone(),
        ));
    }
    while let Some(res) = tasks.join_next().await {
        res??;
    }

    // create a target/api/ dir: this will be zipped & published in pkg/
    //  In addition, exporters, below, will be placed here to complete the API
    let api_dir = package_dir.join("api");
    let target_api_dir = package_dir.join("target").join("api");
    if api_dir.exists() {
        copy_dir(&api_dir, &target_api_dir)?;
    }

    // find non-standard imports/exports -> compositions
    let (importers, exporters) = find_non_standard(package_dir, wasm_paths)?;

    // compose
    for (import, import_paths) in importers {
        let Some(export_path) = exporters.get(&import) else {
            return Err(eyre!(
                "Processes {import_paths:?} required export {import} not found in `pkg/`.",
            ));
        };
        let export_path = export_path.to_str().unwrap();
        for import_path in import_paths {
            let import_path_str = import_path.to_str().unwrap();
            run_command(
                Command::new("wasm-tools")
                    .args([
                        "compose",
                        import_path_str,
                        "-d",
                        export_path,
                        "-o",
                        import_path_str,
                    ]),
                false,
            )?;
        }
    }

    // zip & place API inside of pkg/ to publish API
    if target_api_dir.exists() {
        let zip_path = package_dir.join("pkg").join("api.zip");
        let zip_path = zip_path.to_str().unwrap();
        zip_directory(&target_api_dir, zip_path)?;
    }

    Ok(())
}

#[instrument(level = "trace", skip_all)]
pub async fn execute(
    package_dir: &Path,
    no_ui: bool,
    ui_only: bool,
    skip_deps_check: bool,
    features: &str,
    url: Option<String>,
    download_from: Option<&str>,
    default_world: Option<String>,
    verbose: bool,
) -> Result<()> {
    if !package_dir.join("pkg").exists() {
        if Some(".DS_Store") == package_dir.file_name().and_then(|s| s.to_str()) {
            info!("Skipping build of {:?}", package_dir);
            return Ok(());
        }
        return Err(eyre!(
            "Required `pkg/` dir not found within given input dir {:?} (or cwd, if none given).",
            package_dir,
        )
        .with_suggestion(|| "Please re-run targeting a package."));
    }

    let ui_dir = package_dir.join("ui");
    if !ui_dir.exists() {
        if ui_only {
            return Err(eyre!("kit build: can't build UI: no ui directory exists"));
        } else {
            compile_package(
                package_dir,
                skip_deps_check,
                features,
                url,
                default_world,
                download_from,
                verbose,
            )
            .await
        }
    } else {
        if no_ui {
            return compile_package(
                package_dir,
                skip_deps_check,
                features,
                url,
                default_world,
                download_from,
                verbose,
            )
            .await;
        }

        let deps = check_js_deps()?;
        get_deps(deps, verbose)?;
        let valid_node = get_newest_valid_node_version(None, None)?;

        if ui_only {
            compile_and_copy_ui(package_dir, valid_node, verbose).await
        } else {
            compile_package_and_ui(
                package_dir,
                valid_node,
                skip_deps_check,
                features,
                url,
                default_world,
                download_from,
                verbose,
            )
            .await
        }
    }
}
