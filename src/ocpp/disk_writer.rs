//! Writes OCPP messages to disk as NDJSON (newline-delimited JSON) files.
//!
//! Output goes to `ocpp_datastream/` in the working directory.
//! Each charger gets its own `.jsonl` file named by charger ID.
//!
//! This module is only compiled on native targets (not WASM).

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use bevy::prelude::*;

use super::queue::OcppMessageQueue;

/// Directory where OCPP data stream files are written.
const DATASTREAM_DIR: &str = "ocpp_datastream";

/// Bevy resource that tracks open file handles and the output directory.
#[derive(Resource)]
pub struct OcppDiskWriter {
    /// Root output directory.
    dir: PathBuf,
    /// Whether the output directory has been created.
    dir_created: bool,
    /// Cached file paths per charger_id (avoids repeated PathBuf allocations).
    file_paths: HashMap<String, PathBuf>,
}

impl Default for OcppDiskWriter {
    fn default() -> Self {
        Self {
            dir: PathBuf::from(DATASTREAM_DIR),
            dir_created: false,
            file_paths: HashMap::new(),
        }
    }
}

impl OcppDiskWriter {
    /// Ensure the output directory exists.
    fn ensure_dir(&mut self) {
        if !self.dir_created {
            if let Err(e) = fs::create_dir_all(&self.dir) {
                error!(
                    "OCPP disk writer: failed to create directory {:?}: {}",
                    self.dir, e
                );
            }
            self.dir_created = true;
        }
    }

    /// Get the file path for a charger, caching it for reuse.
    fn file_path(&mut self, charger_id: &str) -> &PathBuf {
        if !self.file_paths.contains_key(charger_id) {
            // Sanitize the charger ID for use as a filename
            let safe_name: String = charger_id
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let path = self.dir.join(format!("{}.jsonl", safe_name));
            self.file_paths.insert(charger_id.to_string(), path);
        }
        &self.file_paths[charger_id]
    }
}

/// Bevy system that drains the disk buffer and appends messages to per-charger
/// `.jsonl` files in the `ocpp_datastream/` directory.
///
/// Runs every frame but only does work when there are buffered messages.
pub fn ocpp_disk_write_system(
    mut queue: ResMut<OcppMessageQueue>,
    mut writer: ResMut<OcppDiskWriter>,
) {
    let messages = queue.drain_disk_buffer();
    if messages.is_empty() {
        return;
    }

    writer.ensure_dir();

    // Group messages by charger_id for efficient batched writes
    let mut grouped: HashMap<String, Vec<String>> = HashMap::new();
    for (charger_id, json) in messages {
        grouped.entry(charger_id).or_default().push(json);
    }

    for (charger_id, msgs) in grouped {
        let path = writer.file_path(&charger_id).clone();
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(mut file) => {
                for json in msgs {
                    if let Err(e) = writeln!(file, "{}", json) {
                        warn!("OCPP disk writer: write error for {:?}: {}", path, e);
                        break;
                    }
                }
            }
            Err(e) => {
                warn!("OCPP disk writer: failed to open {:?}: {}", path, e);
            }
        }
    }
}

/// Startup system that enables disk logging and logs the output path.
pub fn ocpp_disk_writer_init(mut queue: ResMut<OcppMessageQueue>) {
    queue.disk_logging_enabled = true;
    info!(
        "OCPP: Disk logging enabled — writing to {}/",
        DATASTREAM_DIR
    );
}
