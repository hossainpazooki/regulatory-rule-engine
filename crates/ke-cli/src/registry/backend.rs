//! The [`RegistryBackend`] trait seam and its local-FS implementation
//! (ADR 0012). The trait is the seam an S3 backend slots behind in a later
//! gate; Phase 3a ships **local-FS only**, and every object it writes is
//! **non-authoritative** (ADR 0012 §6) — a marker file at the registry root
//! says so loudly.
//!
//! # Layout (ADR 0012 paths, mirrored under one root dir)
//!
//! ```text
//! <root>/
//!   NON_AUTHORITATIVE            marker (ADR 0012 §6)
//!   artifacts/<hash>/artifact.kew
//!   artifacts/<hash>/manifest.json
//!   artifacts/<hash>/schema.json
//!   events/<hash>/<seq:04>-<event_kind>.json   (append-only)
//!   tags/<env>/<tag>.json
//!   policies/<env>/<name>.json
//! ```
//!
//! `<hash>` is the lowercase hex of the 32-byte artifact content hash. Events
//! are JSON files (one per seq) so a reviewer can read the log; the bytes that
//! the hash chain and signatures are computed over are the **postcard**
//! canonical bytes (see [`super::event`]), recovered by re-encoding the decoded
//! event — JSON is the review surface, postcard is the byte contract.
//!
//! Sync I/O only: no async, no tokio, no AWS SDK (CLAUDE.md hard rule).

use crate::registry::event::LifecycleEvent;
use crate::registry::RegistryError;
use std::fs;
use std::path::{Path, PathBuf};

/// The marker file written at the registry root: local-FS objects are
/// non-authoritative (ADR 0012 §6).
pub const NON_AUTHORITATIVE_MARKER: &str = "NON_AUTHORITATIVE";

/// The contents of the non-authoritative marker file.
const NON_AUTHORITATIVE_NOTE: &str = "\
This is a LOCAL-FS registry (Gate 4 Phase 3a). Per ADR 0012 §6, objects here \
are NON-AUTHORITATIVE: signed by a fixed-seed registry-root TEST key, for \
development and testing only. The authoritative registry is S3-backed with an \
HSM-custodied registry root (later gate). Do not treat anything under this \
root as a published, attested, or otherwise authoritative artifact.
";

/// A registry storage backend (ADR 0012). Sync; local-FS now, S3 behind the
/// same trait later. Every method maps I/O failures to a typed
/// [`RegistryError`].
pub trait RegistryBackend {
    /// Store an artifact's `.kew` bytes plus its `manifest.json` /
    /// `schema.json` review views under `artifacts/<hash>/`.
    fn put_artifact(
        &self,
        hash: &[u8; 32],
        kew: &[u8],
        manifest_json: &str,
        schema_json: &str,
    ) -> Result<(), RegistryError>;

    /// Append one lifecycle event under `events/<hash>/<seq:04>-<kind>.json`.
    /// Refuses to overwrite an existing seq (append-only).
    fn append_event(&self, hash: &[u8; 32], event: &LifecycleEvent) -> Result<(), RegistryError>;

    /// Read all events for an artifact, ordered by seq ascending.
    fn read_events(&self, hash: &[u8; 32]) -> Result<Vec<LifecycleEvent>, RegistryError>;

    /// Set a tag pointer `tags/<env>/<tag>.json` to a target hash + the event
    /// reference that justifies it.
    fn put_pointer(
        &self,
        env: &str,
        tag: &str,
        target_hash: &[u8; 32],
        event_ref: &str,
    ) -> Result<(), RegistryError>;

    /// Read a tag pointer; `None` if the tag does not exist.
    fn read_pointer(&self, env: &str, tag: &str) -> Result<Option<TagPointer>, RegistryError>;

    /// List every stored artifact's `(hash, manifest)` for regime resolution.
    fn list_manifests(&self)
        -> Result<Vec<([u8; 32], ke_core::manifest::Manifest)>, RegistryError>;
}

/// A tag pointer object (`tags/<env>/<tag>.json`): which artifact hash the tag
/// resolves to, and the event reference that set it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TagPointer {
    pub env: String,
    pub tag: String,
    /// Lowercase hex of the target artifact hash.
    pub target_hash_hex: String,
    /// A human reference to the event that justified the pointer move.
    pub event_ref: String,
}

impl TagPointer {
    /// Decode `target_hash_hex` back to 32 bytes.
    pub fn target_hash(&self) -> Result<[u8; 32], RegistryError> {
        hex_to_hash(&self.target_hash_hex)
    }
}

/// Lowercase hex of a 32-byte hash (path component + JSON view).
pub fn hex32(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Parse a 64-char lowercase-hex string back to a 32-byte hash.
pub fn hex_to_hash(hex: &str) -> Result<[u8; 32], RegistryError> {
    if hex.len() != 64 {
        return Err(RegistryError::BadHashHex {
            hex: hex.to_string(),
        });
    }
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        let byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| {
            RegistryError::BadHashHex {
                hex: hex.to_string(),
            }
        })?;
        *slot = byte;
    }
    Ok(out)
}

/// A registry backed by a directory tree on the local filesystem (ADR 0012
/// paths). Objects are non-authoritative (ADR 0012 §6).
#[derive(Clone, Debug)]
pub struct LocalFsBackend {
    root: PathBuf,
}

impl LocalFsBackend {
    /// Open (creating if absent) a local-FS registry rooted at `root`. Writes
    /// the non-authoritative marker if it is not already present.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, RegistryError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|e| RegistryError::io("create registry root", e))?;
        let marker = root.join(NON_AUTHORITATIVE_MARKER);
        if !marker.exists() {
            fs::write(&marker, NON_AUTHORITATIVE_NOTE)
                .map_err(|e| RegistryError::io("write non-authoritative marker", e))?;
        }
        Ok(Self { root })
    }

    fn artifacts_dir(&self, hash: &[u8; 32]) -> PathBuf {
        self.root.join("artifacts").join(hex32(hash))
    }

    fn events_dir(&self, hash: &[u8; 32]) -> PathBuf {
        self.root.join("events").join(hex32(hash))
    }

    fn tag_path(&self, env: &str, tag: &str) -> PathBuf {
        self.root.join("tags").join(env).join(format!("{tag}.json"))
    }
}

impl RegistryBackend for LocalFsBackend {
    fn put_artifact(
        &self,
        hash: &[u8; 32],
        kew: &[u8],
        manifest_json: &str,
        schema_json: &str,
    ) -> Result<(), RegistryError> {
        let dir = self.artifacts_dir(hash);
        fs::create_dir_all(&dir).map_err(|e| RegistryError::io("create artifact dir", e))?;
        fs::write(dir.join("artifact.kew"), kew)
            .map_err(|e| RegistryError::io("write artifact.kew", e))?;
        fs::write(dir.join("manifest.json"), manifest_json)
            .map_err(|e| RegistryError::io("write manifest.json", e))?;
        fs::write(dir.join("schema.json"), schema_json)
            .map_err(|e| RegistryError::io("write schema.json", e))?;
        Ok(())
    }

    fn append_event(&self, hash: &[u8; 32], event: &LifecycleEvent) -> Result<(), RegistryError> {
        let dir = self.events_dir(hash);
        fs::create_dir_all(&dir).map_err(|e| RegistryError::io("create events dir", e))?;
        let path = dir.join(format!("{:04}-{}.json", event.seq, event.event_kind));
        if path.exists() {
            return Err(RegistryError::EventExists { seq: event.seq });
        }
        let json = serde_json::to_string_pretty(event).map_err(RegistryError::event_json_encode)?;
        fs::write(&path, json).map_err(|e| RegistryError::io("write event", e))?;
        Ok(())
    }

    fn read_events(&self, hash: &[u8; 32]) -> Result<Vec<LifecycleEvent>, RegistryError> {
        let dir = self.events_dir(hash);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut files: Vec<PathBuf> = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| RegistryError::io("read events dir", e))? {
            let entry = entry.map_err(|e| RegistryError::io("read event entry", e))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                files.push(path);
            }
        }
        // File names start with `<seq:04>-`, so lexicographic order is seq
        // order; `current_state` re-validates seq contiguity regardless.
        files.sort();
        let mut events = Vec::with_capacity(files.len());
        for path in files {
            let bytes = fs::read(&path).map_err(|e| RegistryError::io("read event", e))?;
            let event: LifecycleEvent =
                serde_json::from_slice(&bytes).map_err(RegistryError::event_json_decode)?;
            events.push(event);
        }
        events.sort_by_key(|e| e.seq);
        Ok(events)
    }

    fn put_pointer(
        &self,
        env: &str,
        tag: &str,
        target_hash: &[u8; 32],
        event_ref: &str,
    ) -> Result<(), RegistryError> {
        let path = self.tag_path(env, tag);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| RegistryError::io("create tags dir", e))?;
        }
        let pointer = TagPointer {
            env: env.to_string(),
            tag: tag.to_string(),
            target_hash_hex: hex32(target_hash),
            event_ref: event_ref.to_string(),
        };
        let json =
            serde_json::to_string_pretty(&pointer).map_err(RegistryError::event_json_encode)?;
        fs::write(&path, json).map_err(|e| RegistryError::io("write tag pointer", e))?;
        Ok(())
    }

    fn read_pointer(&self, env: &str, tag: &str) -> Result<Option<TagPointer>, RegistryError> {
        let path = self.tag_path(env, tag);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(&path).map_err(|e| RegistryError::io("read tag pointer", e))?;
        let pointer: TagPointer =
            serde_json::from_slice(&bytes).map_err(RegistryError::event_json_decode)?;
        Ok(Some(pointer))
    }

    fn list_manifests(
        &self,
    ) -> Result<Vec<([u8; 32], ke_core::manifest::Manifest)>, RegistryError> {
        let dir = self.root.join("artifacts");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| RegistryError::io("read artifacts dir", e))? {
            let entry = entry.map_err(|e| RegistryError::io("read artifact entry", e))?;
            let kew_path = entry.path().join("artifact.kew");
            if !kew_path.exists() {
                continue;
            }
            let kew = fs::read(&kew_path).map_err(|e| RegistryError::io("read artifact.kew", e))?;
            let (artifact, _) = ke_artifact::decode_artifact(&kew)
                .map_err(|e| RegistryError::ArtifactDecode(e.to_string()))?;
            out.push((artifact.manifest.artifact_hash, artifact.manifest));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }
}

/// Helper used by tests/CLI: read the marker note (proves a backend dir is a
/// registry root).
pub fn read_marker(root: &Path) -> Result<String, RegistryError> {
    fs::read_to_string(root.join(NON_AUTHORITATIVE_MARKER))
        .map_err(|e| RegistryError::io("read non-authoritative marker", e))
}
