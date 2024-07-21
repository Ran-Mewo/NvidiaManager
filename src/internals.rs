use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use procfs::process::all_processes;
use regex::Regex;
use walkdir::WalkDir;

/// Execute the main logic of the application (Return of true indicates we reverted the changes, return of false indicates we made changes)
pub fn execute(wrapper_dir: &PathBuf, executable_path: &Path) -> Result<bool, Box<dyn Error>> {
    // Check if the path exists, if not then return (This shouldn't happen unless the user deleted the file while the application us running, the application verifies the paths on launch and deletes them accordingly)
    if !executable_path.exists() { return Err(format!("Path {} does not exist", executable_path.display()).into()); }
    
    // Check if the path is a directory, if so apply the logic to all executables in the directory and subdirectories
    if executable_path.is_dir() {
        let paths = find_executables(executable_path);
        println!("Found {} executables in {}", paths.len(), executable_path.display());
        let mut return_result: Result<bool, Box<dyn Error>> = Ok(false);
        for path in paths {
            if path == executable_path { continue; } // Skip the directory itself
            if path == path.with_extension("bak") { continue; } // Skip backup files
            println!("Processing {}", path.display());
            return_result = Ok(execute(wrapper_dir, &path)?);
        }
        return return_result;
    }
    
    // Canonicalize the path to get the full path
    // let target_path = executable_path.canonicalize()?; // BREAKS EVERYTHING FOR SOME REASON
    let target_path = executable_path;
    // Generate a unique name for the wrapper script based on the target path
    let wrapper_name = generate_wrapper_name(original_path(target_path).as_path());

    // Check if the backup exists, if so revert the changes
    if backup_path(target_path).exists() {
        return match revert_changes(target_path, wrapper_dir, &wrapper_name) {
            Err(e) => {
                println!("Failed to revert changes for {}: {}", target_path.display(), e);
                Err(e)
            },
            _ => {
                Ok(true) // Return true as we reverted the changes
            }
        }
    }

    // Create the wrapper script (Enables NVIDIA GPU)
    return match create_wrapper(target_path, wrapper_dir, &wrapper_name) {
        Err(e) => {
            println!("Failed to create wrapper for {}: {}", target_path.display(), e);
            Err(e)
        },
        _ => {
            Ok(false) // Return false as we made changes
        }
    }
}


/// Generate a unique name for the wrapper script by transforming the target path.
pub fn generate_wrapper_name(target_path: &Path) -> String {
    let path_str = target_path.to_str().unwrap();
    // Replace all non-alphanumeric characters with underscores to avoid conflicts
    return format!("wrapper_{}", Regex::new(r"[^a-zA-Z0-9]").unwrap().replace_all(path_str, "_"))
}


/// Create a wrapper script to force the use of the NVIDIA GPU
pub fn create_wrapper(target_path: &Path, wrapper_dir: &Path, wrapper_name: &str) -> Result<(), Box<dyn Error>> {
    // Create the wrapper script
    let wrapper_path = wrapper_dir.join(wrapper_name);
    let mut wrapper_file = fs::File::create(&wrapper_path)?;

    // Write the wrapper script
    write!(
        wrapper_file,
        r#"#!/bin/bash
export __NV_PRIME_RENDER_OFFLOAD=1
export __GLX_VENDOR_LIBRARY_NAME=nvidia
export __VK_LAYER_NV_optimus=NVIDIA_only
exec "{}.bak" "$@"
"#,
        target_path.display()
    )?;

    // Make the wrapper script executable
    Command::new("chmod")
        .arg("+x")
        .arg(&wrapper_path)
        .status()?;

    // Create a backup of the original
    let backup_path = backup_path(target_path);
    fs::rename(target_path, backup_path)?;

    // Create a symbolic link to the wrapper script
    std::os::unix::fs::symlink(&wrapper_path, target_path)?;

    println!("Application {} is now configured to use the NVIDIA GPU by default", target_path.display());
    return Ok(())
}


/// Revert the changes made to the target executable
fn revert_changes(target: &Path, wrapper_dir: &Path, wrapper_name: &str) -> Result<(), Box<dyn Error>> {
    // Get the path to the backup
    let target_path = original_path(target); let target_path = target_path.as_path();
    let backup_path = backup_path(target);

    // Check if the backup exists
    if !backup_path.exists() {
        return Err(format!("No backup found for {}. Cannot revert changes.", target_path.display()).into());
    }

    // Remove the symbolic link
    if let Err(e) = fs::remove_file(target_path) {
        println!("Failed to remove symbolic link for {}: {}", target_path.display(), e);
        return Err(e.into());
    }

    // Restore the original executable from the backup
    if let Err(e) = fs::rename(&backup_path, target_path) {
        println!("Failed to restore original executable for {}: {}", target_path.display(), e);
        return Err(e.into());
    }

    // Remove the wrapper script
    if let Err(e) = fs::remove_file(wrapper_dir.join(wrapper_name)) {
        println!("Failed to remove wrapper script for {}: {}", target_path.display(), e);
        return Err(e.into());
    }

    println!("Reverted changes for {}. Restored original executable.", target_path.display());
    return Ok(())
}


/// Get the path to the backup file
fn backup_path(path: &Path) -> PathBuf {
    // Check if the path has an extension
    let backup_path = if let Some(ext) = path.extension() {
        // If the extension is "bak", return the path as is
        if ext.to_str() == Some("bak") {
            path.to_path_buf()
        } else {
            // Otherwise, append ".bak" to the existing extension
            path.with_extension(format!("{}.bak", ext.to_str().unwrap_or_default()))
        }
    } else {
        // If there is no extension, simply add "bak" as the extension
        path.with_extension("bak")
    };
    return backup_path
}


/// Get the path to the original file
fn original_path(path: &Path) -> PathBuf {
    // Check if the path has an extension
    let original_path = if let Some(ext) = path.extension() {
        // If the extension is "bak", remove it
        if ext.to_str() == Some("bak") {
            path.with_extension("")
        } else {
            // If the extension is not "bak", return the path as is
            path.to_path_buf()
        }
    } else {
        // If there is no extension, return the path as is
        path.to_path_buf()
    };
    return original_path
}


/// Check if a file is executable
fn is_executable(file_path: &Path) -> bool {
    return match fs::metadata(file_path) {
        Ok(metadata) => {
            let permissions = metadata.permissions();
            permissions.mode() & 0o111 != 0 // Check if any executable bit is set
        },
        Err(_) => false,
    }
}


/// Find executable files inside a directory and its subdirectories
fn find_executables(directory: &Path) -> Vec<PathBuf> {
    let mut executables = Vec::new();

    for entry in WalkDir::new(directory).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && is_executable(path) {
            executables.push(path.to_path_buf());
        }
    }

    return executables
}


/// Get a list of all executable paths for running processes
pub fn get_executable_paths() -> Result<HashSet<String>, Box<dyn Error>> {
    // TODO: Maybe filter to only include processes currently on the desktop
    return Ok(
        all_processes()?
          .filter_map(Result::ok) // Automatically filter out Err results and unwrap Ok values
          .filter_map(|proc| return proc.exe().ok()) // Attempt to get the executable path, filter out Err results
          .filter(|exe_path| return exe_path.exists() && has_write_access(exe_path) && !is_system_path(exe_path)) // Check if the path exists, we have write access, and is not a system path
          .filter_map(|exe_path| return exe_path.to_str().map(ToString::to_string)) // Convert to String and filter out None results
          .collect::<HashSet<String>>() // Collect into a HashSet<String>
    )
}


/// Check if a given path is a system path
fn is_system_path(path: &Path) -> bool {
    if let Some(path_str) = path.to_str() {
        return path_str.starts_with("/usr") || path_str.starts_with("/bin") || path_str.starts_with("/sbin");
    }
    return false
}


/// Check if a path has write access
fn has_write_access(path: &Path) -> bool {
    return match fs::metadata(path) {
        Ok(metadata) => {
            let permissions = metadata.permissions();
            permissions.mode() & 0o200 != 0 // Check if user write bit is set
        },
        Err(_) => false,
    }
}