//! The local design library.
//!
//! Layout on disk (sync-friendly: every version is its own immutable file,
//! so two computers saving at once both keep their work):
//!
//! ```text
//! <library>/
//!   library.json                 collections
//!   designs/<id>/
//!     design.json                name, category, tags, favorite, collections, ...
//!     versions/v0003-<ms>.json   {num, created, author, message, params, design}
//!     presets.json               [{name, params}]
//!     pending/<ms>.json          updates from Claude waiting for a compare
//!     thumb.png
//! ```

pub mod config;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(40).collect();
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "design".into()
    } else {
        out
    }
}

/// Write JSON atomically (temp file + rename), creating folders as needed.
pub fn write_json(path: &Path, value: &Json) -> Result<(), String> {
    write_bytes(
        path,
        serde_json::to_string_pretty(value).unwrap().as_bytes(),
    )
}

pub fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn read_json(path: &Path) -> Option<Json> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

pub fn unbase64(s: &str) -> Result<Vec<u8>, String> {
    let s = s.split_once("base64,").map(|(_, b)| b).unwrap_or(s);
    let val = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let clean: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks(4) {
        let mut n = 0u32;
        let mut k: usize = 0;
        for &c in chunk {
            if c == b'=' {
                break;
            }
            n = n << 6 | val(c).ok_or("invalid base64")?;
            k += 1;
        }
        n <<= 6 * (4 - k);
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..k.saturating_sub(1)]);
    }
    Ok(out)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DesignMeta {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub collections: Vec<String>,
    pub created: u64,
    pub modified: u64,
    pub opened: Option<u64>,
    /// "claude", "you", "example" or "import".
    pub source: String,
    /// Arrived from Claude and not opened yet.
    pub is_new: bool,
    /// When it was moved to the trash.
    pub trashed: Option<u64>,
    pub current_version: u32,
    /// Free-form facts the app shows on cards, e.g. `{"size": [x, y, z], "parts": 2}`.
    pub info: Json,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VersionInfo {
    /// File name inside `versions/`.
    pub file: String,
    pub num: u32,
    pub created: u64,
    /// "you" or "claude".
    pub author: String,
    pub message: String,
    pub params: Json,
    /// An update from Claude that was not accepted.
    pub rejected: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub name: String,
    pub params: Json,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub file: String,
    pub created: u64,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    #[serde(flatten)]
    pub meta: DesignMeta,
    pub version_count: usize,
    pub preset_count: usize,
    pub pending_count: usize,
    /// `data:image/png;base64,...` or null.
    pub thumbnail: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    pub meta: DesignMeta,
    pub versions: Vec<VersionInfo>,
    pub presets: Vec<Preset>,
    pub pending: Vec<Pending>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxEvent {
    /// "new" or "update".
    pub kind: String,
    pub id: String,
    pub name: String,
    pub message: String,
}

pub struct Library {
    pub root: PathBuf,
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn design_meta_from(design: &Json) -> (String, String, String, Vec<String>) {
    let m = design.get("meta").cloned().unwrap_or(Json::Null);
    let s = |k: &str| m.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let tags = m
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let name = Some(s("name"))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Untitled design".into());
    (name, s("description"), s("category"), tags)
}

impl Library {
    pub fn open(root: impl Into<PathBuf>) -> Result<Library, String> {
        let root = root.into();
        std::fs::create_dir_all(root.join("designs"))
            .map_err(|e| format!("cannot use {} as the library: {e}", root.display()))?;
        if !root.join("library.json").exists() {
            write_json(
                &root.join("library.json"),
                &json!({"format": "pd-library/1", "collections": []}),
            )?;
        }
        Ok(Library { root })
    }

    fn dir(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty() || id.contains(['/', '\\', '.']) {
            return Err(format!("bad design id {id:?}"));
        }
        let d = self.root.join("designs").join(id);
        if d.join("design.json").exists() {
            Ok(d)
        } else {
            Err(format!("no design {id:?} in the library"))
        }
    }

    pub fn meta(&self, id: &str) -> Result<DesignMeta, String> {
        let d = self.dir(id)?;
        let mut m: DesignMeta = read_json(&d.join("design.json"))
            .and_then(|j| serde_json::from_value(j).ok())
            .ok_or_else(|| format!("design {id} is damaged (design.json)"))?;
        m.id = id.to_string();
        Ok(m)
    }

    fn write_meta(&self, m: &DesignMeta) -> Result<(), String> {
        write_json(
            &self.root.join("designs").join(&m.id).join("design.json"),
            &serde_json::to_value(m).unwrap(),
        )
    }

    pub fn versions(&self, id: &str) -> Result<Vec<VersionInfo>, String> {
        let d = self.dir(id)?.join("versions");
        let mut out = vec![];
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if !name.ends_with(".json") {
                    continue;
                }
                if let Some(mut v) =
                    read_json(&e.path()).and_then(|j| serde_json::from_value::<VersionInfo>(j).ok())
                {
                    v.file = name;
                    out.push(v);
                }
            }
        }
        out.sort_by(|a, b| (b.num, b.created).cmp(&(a.num, a.created)));
        Ok(out)
    }

    pub fn presets(&self, id: &str) -> Result<Vec<Preset>, String> {
        Ok(read_json(&self.dir(id)?.join("presets.json"))
            .and_then(|j| serde_json::from_value(j).ok())
            .unwrap_or_default())
    }

    pub fn pending(&self, id: &str) -> Result<Vec<Pending>, String> {
        let d = self.dir(id)?.join("pending");
        let mut out = vec![];
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let file = e.file_name().to_string_lossy().into_owned();
                if let Some(j) = read_json(&e.path()) {
                    out.push(Pending {
                        file,
                        created: j.get("created").and_then(|c| c.as_u64()).unwrap_or(0),
                        message: j
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("")
                            .to_string(),
                    });
                }
            }
        }
        out.sort_by_key(|p| p.created);
        Ok(out)
    }

    fn thumbnail(&self, id: &str) -> Option<String> {
        let bytes = std::fs::read(self.root.join("designs").join(id).join("thumb.png")).ok()?;
        Some(format!("data:image/png;base64,{}", base64(&bytes)))
    }

    /// Every design, newest change first (trashed ones included; see `meta.trashed`).
    pub fn list(&self) -> Vec<Summary> {
        let mut out = vec![];
        if let Ok(rd) = std::fs::read_dir(self.root.join("designs")) {
            for e in rd.flatten() {
                let id = e.file_name().to_string_lossy().into_owned();
                let Ok(meta) = self.meta(&id) else { continue };
                out.push(Summary {
                    version_count: self.versions(&id).map(|v| v.len()).unwrap_or(0),
                    preset_count: self.presets(&id).map(|p| p.len()).unwrap_or(0),
                    pending_count: self.pending(&id).map(|p| p.len()).unwrap_or(0),
                    thumbnail: self.thumbnail(&id),
                    meta,
                });
            }
        }
        out.sort_by(|a, b| b.meta.modified.cmp(&a.meta.modified));
        out
    }

    pub fn detail(&self, id: &str) -> Result<Detail, String> {
        Ok(Detail {
            meta: self.meta(id)?,
            versions: self.versions(id)?,
            presets: self.presets(id)?,
            pending: self.pending(id)?,
        })
    }

    /// The full version file: `{num, created, author, message, params, design, ...}`.
    pub fn read_version(&self, id: &str, file: &str) -> Result<Json, String> {
        if file.contains(['/', '\\']) {
            return Err("bad version file".into());
        }
        read_json(&self.dir(id)?.join("versions").join(file))
            .ok_or_else(|| format!("version {file} of {id} cannot be read"))
    }

    /// The version the user sees when opening the design.
    pub fn current_version(&self, id: &str) -> Result<Json, String> {
        let meta = self.meta(id)?;
        let versions = self.versions(id)?;
        let v = versions
            .iter()
            .find(|v| v.num == meta.current_version && !v.rejected)
            .or_else(|| versions.iter().find(|v| !v.rejected))
            .ok_or_else(|| format!("design {id} has no versions"))?;
        self.read_version(id, &v.file)
    }

    /// Add a design with its first version. Returns its id.
    pub fn create(
        &self,
        design: &Json,
        params: &Json,
        source: &str,
        message: &str,
        is_new: bool,
    ) -> Result<String, String> {
        let (name, description, category, tags) = design_meta_from(design);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let id = format!("{}-{:x}{:x}", slug(&name), now_ms(), n % 256);
        let now = now_ms();
        let meta = DesignMeta {
            id: id.clone(),
            name,
            description,
            category,
            tags,
            created: now,
            modified: now,
            source: source.to_string(),
            is_new,
            current_version: 0,
            info: json!({}),
            ..Default::default()
        };
        std::fs::create_dir_all(self.root.join("designs").join(&id).join("versions"))
            .map_err(|e| e.to_string())?;
        self.write_meta(&meta)?;
        let author = if source == "claude" { "claude" } else { "you" };
        self.save_version(&id, design, params, author, message, false)?;
        Ok(id)
    }

    /// Save a new version. Rejected Claude updates are kept but never become current.
    pub fn save_version(
        &self,
        id: &str,
        design: &Json,
        params: &Json,
        author: &str,
        message: &str,
        rejected: bool,
    ) -> Result<VersionInfo, String> {
        let d = self.dir(id)?;
        let num = self.versions(id)?.iter().map(|v| v.num).max().unwrap_or(0) + 1;
        let created = now_ms();
        let file = format!("v{num:04}-{created}.json");
        let info = VersionInfo {
            file: file.clone(),
            num,
            created,
            author: author.to_string(),
            message: message.to_string(),
            params: if params.is_object() {
                params.clone()
            } else {
                json!({})
            },
            rejected,
        };
        let mut body = serde_json::to_value(&info).unwrap();
        body["design"] = design.clone();
        write_json(&d.join("versions").join(&file), &body)?;
        if !rejected {
            let mut meta = self.meta(id)?;
            meta.current_version = num;
            meta.modified = created;
            self.write_meta(&meta)?;
        }
        Ok(info)
    }

    /// Change user-editable fields: name, description, category, tags,
    /// favorite, collections, isNew, trashed, opened, info.
    pub fn update_meta(&self, id: &str, patch: &Json) -> Result<DesignMeta, String> {
        let meta = self.meta(id)?;
        let mut j = serde_json::to_value(&meta).unwrap();
        if let Json::Object(p) = patch {
            for (k, v) in p {
                if [
                    "name",
                    "description",
                    "category",
                    "tags",
                    "favorite",
                    "collections",
                    "isNew",
                    "trashed",
                    "opened",
                    "info",
                ]
                .contains(&k.as_str())
                {
                    j[k] = v.clone();
                }
            }
        }
        let mut m: DesignMeta =
            serde_json::from_value(j).map_err(|e| format!("bad change: {e}"))?;
        m.id = id.to_string();
        if patch.get("name").is_some()
            || patch.get("tags").is_some()
            || patch.get("category").is_some()
        {
            m.modified = now_ms();
        }
        self.write_meta(&m)?;
        Ok(m)
    }

    pub fn set_presets(&self, id: &str, presets: &[Preset]) -> Result<(), String> {
        write_json(
            &self.dir(id)?.join("presets.json"),
            &serde_json::to_value(presets).unwrap(),
        )
    }

    pub fn set_thumbnail(&self, id: &str, png: &[u8]) -> Result<(), String> {
        write_bytes(&self.dir(id)?.join("thumb.png"), png)
    }

    pub fn delete_forever(&self, id: &str) -> Result<(), String> {
        let d = self.dir(id)?;
        std::fs::remove_dir_all(&d).map_err(|e| format!("cannot delete {}: {e}", d.display()))
    }

    /// Copy a version into a brand-new design.
    pub fn duplicate(
        &self,
        id: &str,
        file: Option<&str>,
        new_name: &str,
    ) -> Result<String, String> {
        let v = match file {
            Some(f) => self.read_version(id, f)?,
            None => self.current_version(id)?,
        };
        let mut design = v.get("design").cloned().unwrap_or(Json::Null);
        if let Some(m) = design.get_mut("meta").and_then(|m| m.as_object_mut()) {
            m.insert("name".into(), json!(new_name));
        }
        let new_id = self.create(
            &design,
            v.get("params").unwrap_or(&json!({})),
            "you",
            &format!("Copied from “{}”", self.meta(id)?.name),
            false,
        )?;
        let src = self.meta(id)?;
        self.update_meta(&new_id, &json!({"collections": src.collections, "tags": src.tags, "category": src.category, "info": src.info}))?;
        if let Ok(png) = std::fs::read(self.dir(id)?.join("thumb.png")) {
            let _ = self.set_thumbnail(&new_id, &png);
        }
        Ok(new_id)
    }

    pub fn add_pending(&self, id: &str, design: &Json, message: &str) -> Result<String, String> {
        let created = now_ms();
        let file = format!("{created}.json");
        write_json(
            &self.dir(id)?.join("pending").join(&file),
            &json!({"created": created, "message": message, "design": design}),
        )?;
        Ok(file)
    }

    pub fn read_pending(&self, id: &str, file: &str) -> Result<Json, String> {
        if file.contains(['/', '\\']) {
            return Err("bad pending file".into());
        }
        read_json(&self.dir(id)?.join("pending").join(file))
            .ok_or_else(|| "that update is gone".to_string())
    }

    pub fn remove_pending(&self, id: &str, file: &str) -> Result<(), String> {
        if file.contains(['/', '\\']) {
            return Err("bad pending file".into());
        }
        let p = self.dir(id)?.join("pending").join(file);
        std::fs::remove_file(&p).map_err(|e| format!("cannot remove {}: {e}", p.display()))
    }

    pub fn collections(&self) -> Json {
        read_json(&self.root.join("library.json"))
            .and_then(|j| j.get("collections").cloned())
            .filter(|c| c.is_array())
            .unwrap_or_else(|| json!([]))
    }

    pub fn set_collections(&self, collections: &Json) -> Result<(), String> {
        let mut lib = read_json(&self.root.join("library.json"))
            .unwrap_or_else(|| json!({"format": "pd-library/1"}));
        lib["collections"] = collections.clone();
        write_json(&self.root.join("library.json"), &lib)
    }

    /// Turn files Claude dropped in `inbox` into library designs or pending
    /// updates. Handled files are removed; unreadable ones move to `inbox/failed`.
    pub fn process_inbox(&self, inbox: &Path) -> Vec<InboxEvent> {
        let mut events = vec![];
        let Ok(rd) = std::fs::read_dir(inbox) else {
            return events;
        };
        let mut files: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "json"))
            .collect();
        files.sort();
        for f in files {
            let Some(item) = read_json(&f) else {
                let failed = inbox.join("failed");
                let _ = std::fs::create_dir_all(&failed);
                let _ = std::fs::rename(&f, failed.join(f.file_name().unwrap()));
                continue;
            };
            // Older MCP servers wrote the bare design.
            let (kind, design, message, target) = if item.get("format").is_some() {
                ("new".to_string(), item.clone(), String::new(), None)
            } else {
                (
                    item.get("kind")
                        .and_then(|k| k.as_str())
                        .unwrap_or("new")
                        .to_string(),
                    item.get("design").cloned().unwrap_or(Json::Null),
                    item.get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("")
                        .to_string(),
                    item.get("designId")
                        .and_then(|d| d.as_str())
                        .map(String::from),
                )
            };
            let (name, ..) = design_meta_from(&design);
            let result = match (kind.as_str(), target) {
                ("update", Some(id)) if self.dir(&id).is_ok() => self
                    .add_pending(&id, &design, &message)
                    .map(|_| InboxEvent {
                        kind: "update".into(),
                        id,
                        name: name.clone(),
                        message: message.clone(),
                    }),
                _ => {
                    let msg = if message.is_empty() {
                        "Created by Claude".to_string()
                    } else {
                        message.clone()
                    };
                    self.create(&design, &json!({}), "claude", &msg, true)
                        .map(|id| InboxEvent {
                            kind: "new".into(),
                            id,
                            name: name.clone(),
                            message: msg,
                        })
                }
            };
            if let Ok(ev) = result {
                events.push(ev);
                let _ = std::fs::remove_file(&f);
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_lib(tag: &str) -> Library {
        let d = std::env::temp_dir().join(format!(
            "pd-lib-test-{tag}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        Library::open(d).unwrap()
    }

    fn design(name: &str) -> Json {
        json!({"format": "pd/0.1", "meta": {"name": name, "category": "storage/boxes", "tags": ["box"]},
               "parts": [{"id": "a", "shape": {"type": "box", "size": [10, 10, 10]}}]})
    }

    #[test]
    fn base64_roundtrip() {
        for s in [
            &b""[..],
            b"f",
            b"fo",
            b"foo",
            b"foobar",
            &[0, 255, 7, 128, 3],
        ] {
            assert_eq!(unbase64(&base64(s)).unwrap(), s);
        }
        assert_eq!(unbase64("data:image/png;base64,Zm9v").unwrap(), b"foo");
    }

    #[test]
    fn create_save_versions_and_presets() {
        let lib = temp_lib("versions");
        let id = lib
            .create(&design("Box"), &json!({"w": 1}), "you", "Created", false)
            .unwrap();
        assert!(id.starts_with("box-"));
        let v2 = lib
            .save_version(
                &id,
                &design("Box"),
                &json!({"w": 2}),
                "you",
                "width 1 → 2",
                false,
            )
            .unwrap();
        assert_eq!(v2.num, 2);
        lib.save_version(
            &id,
            &design("Box"),
            &json!({"w": 3}),
            "claude",
            "not wanted",
            true,
        )
        .unwrap();
        let d = lib.detail(&id).unwrap();
        assert_eq!(d.versions.len(), 3);
        assert_eq!(d.versions[0].num, 3);
        assert_eq!(
            d.meta.current_version, 2,
            "rejected versions never become current"
        );
        assert_eq!(lib.current_version(&id).unwrap()["params"]["w"], 2);
        lib.set_presets(
            &id,
            &[Preset {
                name: "Small".into(),
                params: json!({"w": 1}),
            }],
        )
        .unwrap();
        assert_eq!(lib.presets(&id).unwrap()[0].name, "Small");
        let m = lib
            .update_meta(
                &id,
                &json!({"favorite": true, "tags": ["a", "b"], "evil": 1}),
            )
            .unwrap();
        assert!(m.favorite);
        assert_eq!(m.tags, vec!["a", "b"]);
        let list = lib.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].version_count, 3);
        let copy = lib.duplicate(&id, None, "Box copy").unwrap();
        assert_eq!(lib.meta(&copy).unwrap().name, "Box copy");
        lib.delete_forever(&id).unwrap();
        assert_eq!(lib.list().len(), 1);
        assert!(lib.meta("../etc").is_err());
    }

    #[test]
    fn inbox_creates_designs_and_pending_updates() {
        let lib = temp_lib("inbox");
        let inbox = lib.root.join("test-inbox");
        std::fs::create_dir_all(&inbox).unwrap();
        write_json(
            &inbox.join("1-a.json"),
            &json!({"kind": "new", "design": design("Pen cup")}),
        )
        .unwrap();
        write_json(&inbox.join("2-legacy.pd.json"), &design("Old style")).unwrap();
        std::fs::write(inbox.join("3-bad.json"), "{nope").unwrap();
        let ev = lib.process_inbox(&inbox);
        assert_eq!(ev.len(), 2);
        assert!(ev.iter().all(|e| e.kind == "new"));
        let id = ev[0].id.clone();
        assert!(lib.meta(&id).unwrap().is_new);
        assert_eq!(lib.versions(&id).unwrap()[0].author, "claude");
        write_json(&inbox.join("4-u.json"), &json!({"kind": "update", "designId": id, "message": "taller", "design": design("Pen cup")})).unwrap();
        let ev = lib.process_inbox(&inbox);
        assert_eq!(ev[0].kind, "update");
        let p = lib.pending(&id).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].message, "taller");
        assert_eq!(
            lib.read_pending(&id, &p[0].file).unwrap()["design"]["meta"]["name"],
            "Pen cup"
        );
        lib.remove_pending(&id, &p[0].file).unwrap();
        assert!(lib.pending(&id).unwrap().is_empty());
        assert!(inbox.join("failed").join("3-bad.json").exists());
        assert_eq!(
            std::fs::read_dir(&inbox)
                .unwrap()
                .flatten()
                .filter(|e| e.path().is_file())
                .count(),
            0
        );
    }

    #[test]
    fn collections_roundtrip() {
        let lib = temp_lib("collections");
        lib.set_collections(&json!([{"id": "desk", "name": "Desk setup"}]))
            .unwrap();
        assert_eq!(lib.collections()[0]["name"], "Desk setup");
    }
}
