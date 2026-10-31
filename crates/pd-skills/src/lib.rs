//! The skill catalog. Core skills are always included; the user picks
//! optional ones. A selection can be exported as one tailored skill (a folder,
//! or a .zip for apps that install skills), as a single pasteable prompt, or
//! skill by skill.
//!
//! Skills are plain folders: `<id>/SKILL.md` is what the LLM reads, and an
//! optional `<id>/skill.json` plus a thumbnail are what the user sees in the
//! catalog. The app reads them from a folder on disk every time, so adding or
//! editing a skill needs no rebuild; the built-in set is embedded in the binary
//! and copied into that folder (see [`sync_builtin`]).

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

mod embedded;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillInfo {
    /// The skill's folder name.
    pub id: String,
    pub title: String,
    /// The catalog section it is listed under ("Connectors").
    pub category: String,
    #[serde(default)]
    pub core: bool,
    /// Skills in the same group are alternatives (e.g. one connector type).
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    /// What the skill does, for the user.
    pub summary: String,
    /// Sort key: lower comes first in the catalog and in bundles.
    pub order: i64,
    /// Picture for the catalog, relative to the skill's folder.
    pub thumbnail: Option<String>,
}

/// `skill.json`: everything is optional.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillMeta {
    title: Option<String>,
    category: Option<String>,
    #[serde(default)]
    core: bool,
    group: Option<String>,
    #[serde(default)]
    requires: Vec<String>,
    summary: Option<String>,
    order: Option<i64>,
    thumbnail: Option<String>,
}

/// Files in a skill's folder that are for the app, not the LLM.
const META_FILE: &str = "skill.json";
const THUMBNAILS: [&str; 4] = [
    "thumbnail.png",
    "thumbnail.jpg",
    "thumbnail.jpeg",
    "thumbnail.webp",
];

pub struct Catalog {
    pub skills: Vec<SkillInfo>,
    /// Skills that could not be read, and other things worth fixing, in words
    /// for the person editing the skills folder.
    pub problems: Vec<String>,
    /// path relative to the skills root -> contents
    files: BTreeMap<String, Vec<u8>>,
}

/// A reference photo that goes with the brief.
#[derive(Clone, Debug, Default)]
pub struct Photo {
    /// The original file name; bundles keep only its extension.
    pub name: String,
    /// What the photo shows, in the user's words.
    pub caption: Option<String>,
    pub bytes: Vec<u8>,
}

/// The user's printer, so the design is sized for its bed.
#[derive(Clone, Debug)]
pub struct PrinterNote {
    pub name: String,
    pub bed: [f64; 3],
    pub nozzle: f64,
}

#[derive(Clone, Debug, Default)]
pub struct BundleRequest {
    /// Optional skill ids to add to the core skills.
    pub optional: Vec<String>,
    /// A short name for the design ("Headphone hook").
    pub title: Option<String>,
    /// What the user wants, in their words.
    pub brief: Option<String>,
    /// Reference photos to include with the brief.
    pub photos: Vec<Photo>,
    pub printer: Option<PrinterNote>,
}

const MAX_PHOTOS: usize = 10;

fn text_field(v: &Json, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|t| t.as_str())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
}

/// Standard or URL-safe base64, with or without a `data:...;base64,` prefix.
fn unbase64(s: &str) -> Result<Vec<u8>, String> {
    let s = s.split_once("base64,").map_or(s, |(_, d)| d);
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\r' | b'\n' | b' ' => continue,
            _ => return Err("photo data is not valid base64".into()),
        };
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

impl BundleRequest {
    /// From the app: `{"optional": [ids], "title", "brief", "printer": {"name",
    /// "bed": [x, y, z], "nozzle"}, "photos": [{"name", "caption", "data"}]}`,
    /// where `data` is base64 or a data URL.
    pub fn from_json(v: &Json) -> Result<BundleRequest, String> {
        let optional = v
            .get("optional")
            .and_then(|o| o.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let printer = v.get("printer").filter(|p| p.is_object()).and_then(|p| {
            let bed = p.get("bed")?.as_array()?;
            let n = |i: usize| bed.get(i).and_then(|x| x.as_f64());
            Some(PrinterNote {
                name: text_field(p, "name").unwrap_or_else(|| "My printer".into()),
                bed: [n(0)?, n(1)?, n(2)?],
                nozzle: p.get("nozzle").and_then(|x| x.as_f64()).unwrap_or(0.4),
            })
        });
        let mut photos = vec![];
        for (i, p) in v
            .get("photos")
            .and_then(|p| p.as_array())
            .into_iter()
            .flatten()
            .enumerate()
        {
            let data = p
                .get("data")
                .and_then(|d| d.as_str())
                .ok_or_else(|| format!("photo {} has no data", i + 1))?;
            photos.push(Photo {
                name: text_field(p, "name").unwrap_or_else(|| format!("photo-{}.jpg", i + 1)),
                caption: text_field(p, "caption"),
                bytes: unbase64(data)?,
            });
        }
        Ok(BundleRequest {
            optional,
            title: text_field(v, "title"),
            brief: text_field(v, "brief"),
            photos,
            printer,
        })
    }

    fn has_brief(&self) -> bool {
        self.title.is_some()
            || self.brief.is_some()
            || !self.photos.is_empty()
            || self.printer.is_some()
    }
}

fn strip_frontmatter(s: &str) -> &str {
    if let Some(rest) = s.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            return rest[end + 4..].trim_start_matches(['\r', '\n']);
        }
    }
    s
}

/// Demote markdown headings by one level and point relative links into the
/// bundled skill's per-skill folders.
fn rebase(body: &str, id: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
        }
        let l = if !in_code && line.starts_with('#') {
            format!("#{line}")
        } else {
            line.to_string()
        };
        out.push_str(
            &l.replace("references/", &format!("references/{id}/"))
                .replace("examples/", &format!("examples/{id}/")),
        );
        out.push('\n');
    }
    out
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-')
        .chars()
        .take(40)
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// The photo's real type from its first bytes (the app converts photos, so
/// the original name can say otherwise), else from its name.
fn photo_ext(p: &Photo) -> String {
    let b = &p.bytes;
    if b.starts_with(&[0xff, 0xd8, 0xff]) {
        return "jpg".into();
    }
    if b.starts_with(b"\x89PNG") {
        return "png".into();
    }
    if b.starts_with(b"GIF8") {
        return "gif".into();
    }
    if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        return "webp".into();
    }
    match p.name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()) {
        Some(e) if ["jpg", "jpeg", "png", "webp", "gif"].contains(&e.as_str()) => e,
        _ => "jpg".into(),
    }
}

fn fmt_mm(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {one}s")
    }
}

/// Files under one folder in a zip, as skill apps expect (`<name>/SKILL.md`).
fn zip_folder(root: &str, files: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut z = zip::ZipWriter::new(&mut buf);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let stored = deflated.compression_method(zip::CompressionMethod::Stored);
    for (path, bytes) in files {
        // photos are compressed already
        let o = if path.starts_with("brief/") {
            stored
        } else {
            deflated
        };
        z.start_file(format!("{root}/{path}"), o)
            .map_err(|e| e.to_string())?;
        z.write_all(bytes).map_err(|e| e.to_string())?;
    }
    z.finish().map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}

fn write_files(out: &Path, files: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    for (rel, bytes) in files {
        let p = out.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&p, bytes).map_err(|e| format!("cannot write {}: {e}", p.display()))?;
    }
    Ok(())
}

/// Every file under `dir`, keyed by its path relative to `dir` with `/`.
/// Hidden files and folders (and the sync record) are skipped.
fn read_tree(dir: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, Vec<u8>>) -> std::io::Result<()> {
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, out)?;
            } else {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&p)?);
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    walk(dir, dir, &mut files)
        .map_err(|e| format!("cannot read skills from {}: {e}", dir.display()))?;
    Ok(files)
}

/// `name` and `description` from a SKILL.md's frontmatter.
fn frontmatter(md: &str) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut desc = None;
    if let Some(rest) = md.trim_start_matches('\u{feff}').strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            for line in rest[..end].lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let v = v.trim().trim_matches('"').trim().to_string();
                    match k.trim() {
                        "name" if !v.is_empty() => name = Some(v),
                        "description" if !v.is_empty() => desc = Some(v),
                        _ => {}
                    }
                }
            }
        }
    }
    (name, desc)
}

/// "connector-dovetail" -> "Connector dovetail"
fn title_from_id(id: &str) -> String {
    let t = id.replace(['-', '_'], " ");
    let mut c = t.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => t,
    }
}

/// One skill's catalog entry, and warnings that don't stop it being used.
fn skill_info(
    id: &str,
    skill_md: &[u8],
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<(SkillInfo, Vec<String>), String> {
    let mut warnings = vec![];
    let md = String::from_utf8_lossy(skill_md);
    let (name, description) = frontmatter(&md);
    match &name {
        None => warnings.push(format!(
            "{id}: SKILL.md should start with a frontmatter block (---, name: {id}, description: ..., ---)"
        )),
        Some(n) if n != id => warnings.push(format!(
            "{id}: the name in SKILL.md is \"{n}\"; it should match the folder name \"{id}\""
        )),
        _ => {}
    }
    let meta: SkillMeta = match files.get(&format!("{id}/{META_FILE}")) {
        Some(b) => serde_json::from_slice(b).map_err(|e| {
            format!("{id}: {META_FILE} is not valid: {e}. The skill is left out until it's fixed.")
        })?,
        None => SkillMeta::default(),
    };
    let thumbnail = match meta.thumbnail {
        Some(t) => {
            if !files.contains_key(&format!("{id}/{t}")) {
                warnings.push(format!("{id}: the thumbnail \"{t}\" is missing"));
            }
            Some(t)
        }
        None => THUMBNAILS
            .iter()
            .find(|t| files.contains_key(&format!("{id}/{t}")))
            .map(|t| t.to_string()),
    };
    let summary = meta.summary.or(description).unwrap_or_default();
    Ok((
        SkillInfo {
            id: id.to_string(),
            title: meta.title.unwrap_or_else(|| title_from_id(id)),
            category: meta.category.unwrap_or_else(|| "Other".into()),
            core: meta.core,
            group: meta.group,
            requires: meta.requires,
            summary,
            order: meta.order.unwrap_or(1000),
            thumbnail,
        },
        warnings,
    ))
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// A stable 64-bit hash (FNV-1a), to tell whether a file was edited.
fn fingerprint(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// Records which built-in files were written into a skills folder, and what
/// they contained, so later versions of the app can tell edits from originals.
const SYNC_FILE: &str = ".builtin-skills.json";

/// Bring the built-in skills into the user's skills folder (created if needed):
/// - a built-in file that isn't there yet is added, unless the user deleted
///   that skill's folder;
/// - a file still exactly as the app wrote it is updated when the built-in
///   version changes;
/// - a file the user edited or deleted is left alone, and so is every skill
///   the user added.
///
/// With `force`, every built-in file is written again (restoring the
/// originals); skills the user added are still left alone.
pub fn sync_builtin(dir: &Path, force: bool) -> Result<usize, String> {
    let err = |e: std::io::Error| format!("cannot update the skills folder {}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(err)?;
    let record_path = dir.join(SYNC_FILE);
    let mut record: BTreeMap<String, String> = std::fs::read(&record_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let known_skills: Vec<String> = record
        .keys()
        .filter_map(|k| k.split_once('/').map(|(d, _)| d.to_string()))
        .collect();
    let mut written = 0;
    for (rel, bytes) in embedded::FILES {
        let path = dir.join(rel);
        let new = fingerprint(bytes);
        let on_disk = std::fs::read(&path).ok();
        let write = if force {
            on_disk.as_deref() != Some(*bytes)
        } else {
            match (record.get(*rel), &on_disk) {
                // unchanged since the app wrote it: take the new version
                (Some(old), Some(d)) => fingerprint(d) == *old && *old != new,
                // the user deleted it
                (Some(_), None) => false,
                // the user's own file with the same name
                (None, Some(_)) => false,
                // new built-in file, unless its whole skill was deleted
                (None, None) => match rel.split_once('/') {
                    Some((skill, _)) => {
                        !known_skills.iter().any(|k| k == skill) || dir.join(skill).is_dir()
                    }
                    None => true,
                },
            }
        };
        if write {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(err)?;
            }
            std::fs::write(&path, bytes).map_err(err)?;
            written += 1;
        }
        if write || on_disk.as_deref() == Some(*bytes) {
            record.insert(rel.to_string(), new);
        }
    }
    let text = serde_json::to_string_pretty(&record).unwrap();
    if std::fs::read_to_string(&record_path).ok().as_deref() != Some(text.as_str()) {
        std::fs::write(&record_path, text).map_err(err)?;
    }
    Ok(written)
}

impl Catalog {
    /// The built-in skills embedded in the binary.
    pub fn builtin() -> Catalog {
        let files = embedded::FILES
            .iter()
            .map(|(p, b)| (p.to_string(), b.to_vec()))
            .collect();
        Catalog::from_files(files)
    }

    /// Load from a skills folder, or the built-in skills when there is none.
    pub fn load(dir: Option<&Path>) -> Result<Catalog, String> {
        match dir {
            Some(d) => Ok(Catalog::from_files(read_tree(d)?)),
            None => Ok(Catalog::builtin()),
        }
    }

    /// The skills folder the user edits: missing built-in skills are added and
    /// untouched ones updated first ([`sync_builtin`]), then it is read.
    pub fn installed(dir: &Path) -> Result<Catalog, String> {
        let sync = sync_builtin(dir, false);
        let mut c = Catalog::from_files(read_tree(dir)?);
        if let Err(e) = sync {
            c.problems.insert(0, e);
        }
        Ok(c)
    }

    /// Every folder with a `SKILL.md` is a skill. Broken ones are left out and
    /// reported in `problems` instead of failing the whole catalog.
    fn from_files(mut files: BTreeMap<String, Vec<u8>>) -> Catalog {
        // The component reference is always regenerated from the engine.
        if files.contains_key("core-parametric/SKILL.md") {
            files.insert(
                "core-parametric/references/components.md".into(),
                pd_core::spec::components_markdown().into_bytes(),
            );
        }
        let mut skills = vec![];
        let mut problems = vec![];
        for (path, bytes) in &files {
            let Some(id) = path.strip_suffix("/SKILL.md") else {
                continue;
            };
            if id.contains('/') {
                continue;
            }
            match skill_info(id, bytes, &files) {
                Ok((info, warnings)) => {
                    skills.push(info);
                    problems.extend(warnings);
                }
                Err(e) => problems.push(e),
            }
        }
        skills.sort_by(|a, b| (a.order, &a.id).cmp(&(b.order, &b.id)));
        let ids: Vec<String> = skills.iter().map(|s| s.id.clone()).collect();
        for s in &mut skills {
            s.requires.retain(|r| {
                let known = ids.contains(r);
                if !known {
                    problems.push(format!(
                        "{}: requires \"{r}\", but there is no skill folder called that",
                        s.id
                    ));
                }
                known
            });
        }
        Catalog {
            skills,
            problems,
            files,
        }
    }

    /// The picture shown in the catalog, as a data URL.
    pub fn thumbnail_data_url(&self, id: &str) -> Option<String> {
        let rel = self.get(id)?.thumbnail.as_ref()?;
        let bytes = self.files.get(&format!("{id}/{rel}"))?;
        let mime = match rel.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()) {
            Some(e) if e == "png" => "image/png",
            Some(e) if e == "webp" => "image/webp",
            Some(e) if e == "svg" => "image/svg+xml",
            _ => "image/jpeg",
        };
        Some(format!("data:{mime};base64,{}", base64(bytes)))
    }

    pub fn get(&self, id: &str) -> Option<&SkillInfo> {
        self.skills.iter().find(|s| s.id == id)
    }

    pub fn text(&self, path: &str) -> Option<String> {
        self.files
            .get(path)
            .map(|b| String::from_utf8_lossy(b).into_owned())
    }

    /// The files of one skill that are for the LLM (not its `skill.json` or
    /// catalog thumbnail).
    fn files_of(&self, id: &str) -> Vec<(&str, &Vec<u8>)> {
        let prefix = format!("{id}/");
        let thumb = self.get(id).and_then(|s| s.thumbnail.as_deref());
        self.files
            .iter()
            .filter_map(|(p, b)| p.strip_prefix(&prefix).map(|r| (r, b)))
            .filter(|(r, _)| *r != META_FILE && Some(*r) != thumb)
            .collect()
    }

    /// Core skills plus the requested optional ones (and what they require),
    /// in catalog order. Rejects unknown ids and two choices from one group.
    pub fn resolve(&self, optional: &[String]) -> Result<Vec<&SkillInfo>, String> {
        let mut want: Vec<String> = self
            .skills
            .iter()
            .filter(|s| s.core)
            .map(|s| s.id.clone())
            .collect();
        let mut queue: Vec<String> = optional.to_vec();
        while let Some(id) = queue.pop() {
            let s = self.get(&id).ok_or_else(|| {
                let ids: Vec<&str> = self.skills.iter().map(|s| s.id.as_str()).collect();
                match pd_core::issues::suggest(&id, ids.iter().copied()) {
                    Some(x) => format!("unknown skill \"{id}\" (did you mean \"{x}\"?)"),
                    None => format!("unknown skill \"{id}\"; available: {}", ids.join(", ")),
                }
            })?;
            if !want.contains(&s.id) {
                want.push(s.id.clone());
                queue.extend(s.requires.iter().cloned());
            }
        }
        let chosen: Vec<&SkillInfo> = self
            .skills
            .iter()
            .filter(|s| want.contains(&s.id))
            .collect();
        for g in chosen.iter().filter_map(|s| s.group.as_deref()) {
            let same: Vec<&str> = chosen
                .iter()
                .filter(|s| s.group.as_deref() == Some(g))
                .map(|s| s.id.as_str())
                .collect();
            if same.len() > 1 {
                return Err(format!(
                    "pick only one \"{g}\" skill, not {}",
                    same.join(" and ")
                ));
            }
        }
        Ok(chosen)
    }

    /// One skill's files, as they are in the catalog.
    pub fn skill_files(&self, id: &str) -> Result<BTreeMap<String, Vec<u8>>, String> {
        if self.get(id).is_none() {
            return Err(format!("unknown skill \"{id}\""));
        }
        Ok(self
            .files_of(id)
            .into_iter()
            .map(|(rel, bytes)| (rel.to_string(), bytes.clone()))
            .collect())
    }

    /// Write one skill's folder as-is.
    pub fn export_one(&self, id: &str, out: &Path) -> Result<(), String> {
        write_files(out, &self.skill_files(id)?)
    }

    /// One skill as a .zip holding its folder.
    pub fn skill_zip(&self, id: &str) -> Result<Vec<u8>, String> {
        zip_folder(id, &self.skill_files(id)?)
    }

    /// `labels`: how each photo is referred to, e.g. "Photo 1 (`brief/photo-1.jpg`)".
    fn merged_body(&self, chosen: &[&SkillInfo], req: &BundleRequest, labels: &[String]) -> String {
        let mut s = String::new();
        s.push_str("# Parametric design skill (tailored bundle)\n\n");
        s.push_str("This skill was assembled by ParametricDesign from these skills: ");
        s.push_str(
            &chosen
                .iter()
                .map(|c| format!("**{}**", c.title))
                .collect::<Vec<_>>()
                .join(", "),
        );
        s.push_str(".\nFollow all of them together. When two sections seem to disagree, the more specific (later) skill wins.\n\n");
        if req.has_brief() {
            s.push_str("## The user's design brief\n\n");
            if let Some(t) = &req.title {
                s.push_str(&format!("**What to design:** {t}\n\n"));
            }
            if let Some(b) = &req.brief {
                s.push_str(b.trim());
                s.push_str("\n\n");
            }
            for (label, p) in labels.iter().zip(&req.photos) {
                match &p.caption {
                    Some(c) => s.push_str(&format!("- {label}: {c}\n")),
                    None => s.push_str(&format!("- {label}\n")),
                }
            }
            if let Some(p) = &req.printer {
                s.push_str(&format!(
                    "- Printer: {} (bed {} × {} × {} mm, {} mm nozzle). Every part must fit on this bed.\n",
                    p.name,
                    fmt_mm(p.bed[0]),
                    fmt_mm(p.bed[1]),
                    fmt_mm(p.bed[2]),
                    fmt_mm(p.nozzle)
                ));
            }
            if !labels.is_empty() || req.printer.is_some() {
                s.push('\n');
            }
            s.push_str("Design exactly this. Do not ask follow-up questions unless something essential is missing; expose anything uncertain as a parameter instead.\n\n");
        }
        for c in chosen {
            let body = self.text(&format!("{}/SKILL.md", c.id)).unwrap_or_default();
            s.push_str(&rebase(strip_frontmatter(&body), &c.id));
            s.push('\n');
        }
        s
    }

    /// The tailored skill's name: from the design's title, else the first line
    /// of the brief, else the optional skills.
    pub fn bundle_name(&self, req: &BundleRequest) -> Result<String, String> {
        let chosen = self.resolve(&req.optional)?;
        let extra: Vec<&str> = chosen
            .iter()
            .filter(|c| !c.core)
            .map(|c| c.title.as_str())
            .collect();
        let name = match topic(req) {
            Some(t) => format!("parametric-design-{}", slug(&t)),
            None if !extra.is_empty() => format!("parametric-design-{}", slug(&extra.join(" "))),
            None => "parametric-design".to_string(),
        };
        Ok(name
            .chars()
            .take(64)
            .collect::<String>()
            .trim_end_matches('-')
            .to_string())
    }

    /// The tailored skill as files (relative path -> contents) and its name.
    pub fn bundle_files(
        &self,
        req: &BundleRequest,
    ) -> Result<(String, BTreeMap<String, Vec<u8>>), String> {
        if req.photos.len() > MAX_PHOTOS {
            return Err(format!("use at most {MAX_PHOTOS} photos"));
        }
        let chosen = self.resolve(&req.optional)?;
        let name = self.bundle_name(req)?;
        let extra: Vec<&str> = chosen
            .iter()
            .filter(|c| !c.core)
            .map(|c| c.title.as_str())
            .collect();
        let mut desc = String::from("Writes ParametricDesign (pd/0.1) JSON designs: parametric, printable 3D models with sliders, built from components placed by anchors");
        if !extra.is_empty() {
            desc.push_str(&format!(", including {}", extra.join(", ").to_lowercase()));
        }
        desc.push_str(
            ". Use whenever the user wants a 3D-printable object designed, changed or fixed",
        );
        if let Some(t) = topic(req) {
            let short: String = t.chars().take(160).collect();
            desc.push_str(&format!(
                ", in particular: {}",
                short.replace(['\n', '"'], " ")
            ));
        }
        desc.push('.');
        let desc: String = desc.chars().take(1000).collect();

        let mut files = BTreeMap::new();
        let mut labels = vec![];
        for (i, p) in req.photos.iter().enumerate() {
            let path = format!("brief/photo-{}.{}", i + 1, photo_ext(p));
            labels.push(format!("Photo {} (`{path}`)", i + 1));
            files.insert(path, p.bytes.clone());
        }
        let body = self.merged_body(&chosen, req, &labels);
        files.insert(
            "SKILL.md".into(),
            format!("---\nname: {name}\ndescription: {desc}\n---\n\n{body}").into_bytes(),
        );
        for c in &chosen {
            for (rel, bytes) in self.files_of(&c.id) {
                if rel == "SKILL.md" {
                    continue;
                }
                let path = match rel.split_once('/') {
                    Some((dir, rest)) => format!("{dir}/{}/{rest}", c.id),
                    None => format!("references/{}/{rel}", c.id),
                };
                files.insert(path, bytes.clone());
            }
        }
        Ok((name, files))
    }

    /// Write a single tailored skill folder. Returns its skill name.
    pub fn bundle(&self, req: &BundleRequest, out: &Path) -> Result<String, String> {
        let (name, files) = self.bundle_files(req)?;
        write_files(out, &files)?;
        Ok(name)
    }

    /// The tailored skill as a .zip holding its folder, and its name.
    pub fn bundle_zip(&self, req: &BundleRequest) -> Result<(String, Vec<u8>), String> {
        let (name, files) = self.bundle_files(req)?;
        let zip = zip_folder(&name, &files)?;
        Ok((name, zip))
    }

    /// Everything as one Markdown prompt (references inlined), for chat LLMs
    /// that cannot load skill folders. Photos are attached to the chat.
    pub fn prompt(&self, req: &BundleRequest) -> Result<String, String> {
        let chosen = self.resolve(&req.optional)?;
        let labels: Vec<String> = (1..=req.photos.len())
            .map(|i| format!("Photo {i} (attached to this chat)"))
            .collect();
        let mut s = self.merged_body(&chosen, req, &labels);
        s.push_str("\n---\n\n# Reference files\n\n");
        for c in &chosen {
            for (rel, bytes) in self.files_of(&c.id) {
                if rel == "SKILL.md" || !(rel.ends_with(".md") || rel.ends_with(".json")) {
                    continue;
                }
                let shown = match rel.split_once('/') {
                    Some((dir, rest)) => format!("{dir}/{}/{rest}", c.id),
                    None => format!("references/{}/{rel}", c.id),
                };
                let text = String::from_utf8_lossy(bytes);
                if rel.ends_with(".json") {
                    s.push_str(&format!(
                        "## File: {shown}\n\n```json\n{}\n```\n\n",
                        text.trim()
                    ));
                } else {
                    s.push_str(&format!("## File: {shown}\n\n{}\n\n", rebase(&text, &c.id)));
                }
            }
        }
        Ok(s)
    }
}

fn topic(req: &BundleRequest) -> Option<String> {
    req.title.clone().or_else(|| {
        req.brief
            .as_deref()
            .and_then(|b| b.lines().map(str::trim).find(|l| !l.is_empty()))
            .map(String::from)
    })
}

fn what(req: &BundleRequest) -> String {
    match &req.title {
        Some(t) => format!("“{t}”"),
        None => "what its brief describes".into(),
    }
}

/// What to paste into a chat once the tailored skill is installed.
pub fn skill_starter(name: &str, req: &BundleRequest) -> String {
    let mut s = format!("Use the {name} skill to design {}.", what(req));
    match req.photos.len() {
        0 => {}
        n => s.push_str(&format!(
            " I've attached the {} its brief mentions, in the same order.",
            plural(n, "photo")
        )),
    }
    s.push_str(" Follow the skill's workflow and give me the finished design.");
    s
}

/// What to send with the one-prompt file attached.
pub fn prompt_starter(file_name: &str, req: &BundleRequest) -> String {
    let mut s = format!(
        "Read the attached {file_name} and follow it: it explains how to write a ParametricDesign design, and its brief says what I want."
    );
    match req.photos.len() {
        0 => {}
        n => s.push_str(&format!(
            " The {} I've attached go with the brief, in the same order.",
            plural(n, "photo")
        )),
    }
    s.push_str(" Give me the finished design as one ```json block.");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `src/embedded.rs` should contain for the current skills folder.
    fn embedded_source() -> String {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.file_name().unwrap().to_string_lossy().starts_with('.') {
                    continue;
                }
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    out.push(
                        p.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills");
        let mut files = vec![];
        walk(&root, &root, &mut files);
        files.sort();
        let mut s = String::from(
            "// The skills/ folder, embedded in the binary. A list instead of a build script,\n\
             // so building the app compiles and runs nothing extra. After adding or removing\n\
             // a file under skills/, regenerate it: PD_REGEN=1 cargo test -p pd-skills\n\n\
             #[rustfmt::skip]\n\
             pub static FILES: &[(&str, &[u8])] = &[\n",
        );
        for f in &files {
            s.push_str(&format!(
                "    (\"{f}\", include_bytes!(\"../../../skills/{f}\")),\n"
            ));
        }
        s.push_str("];\n");
        s
    }

    #[test]
    fn embedded_list_matches_the_skills_folder() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/embedded.rs");
        let want = embedded_source();
        if std::fs::read_to_string(&path).unwrap_or_default() != want {
            if std::env::var_os("PD_REGEN").is_some() {
                std::fs::write(&path, want).unwrap();
            } else {
                panic!("src/embedded.rs no longer matches skills/: run PD_REGEN=1 cargo test -p pd-skills");
            }
        }
    }

    #[test]
    fn catalog_loads_and_every_skill_has_a_skill_md() {
        let c = Catalog::load(None).unwrap();
        assert!(c.skills.iter().any(|s| s.core));
        for s in &c.skills {
            let md = c
                .text(&format!("{}/SKILL.md", s.id))
                .unwrap_or_else(|| panic!("{} has no SKILL.md", s.id));
            assert!(md.starts_with("---\nname: "), "{} frontmatter", s.id);
            assert!(
                md.contains(&format!("name: {}\n", s.id)),
                "{} name must match its id",
                s.id
            );
        }
    }

    #[test]
    fn resolve_rules() {
        let c = Catalog::load(None).unwrap();
        let r = c.resolve(&["connector-dovetail".into()]).unwrap();
        assert!(
            r.iter().any(|s| s.id == "multi-part"),
            "requires pulls in multi-part"
        );
        assert!(c
            .resolve(&["connector-dovetail".into(), "connector-pin".into()])
            .is_err());
        assert!(c.resolve(&["nope".into()]).is_err());
    }

    #[test]
    fn bundle_writes_one_skill() {
        let c = Catalog::load(None).unwrap();
        let dir = std::env::temp_dir().join(format!("pd-bundle-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let name = c
            .bundle(
                &BundleRequest {
                    optional: vec!["connector-dovetail".into()],
                    brief: Some("A 300 mm desk tray".into()),
                    ..Default::default()
                },
                &dir,
            )
            .unwrap();
        assert_eq!(name, "parametric-design-a-300-mm-desk-tray");
        let md = std::fs::read_to_string(dir.join("SKILL.md")).unwrap();
        assert!(md.contains("A 300 mm desk tray"));
        assert!(dir
            .join("references/core-parametric/components.md")
            .exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn request_from_the_app_becomes_a_zip_with_photos() {
        let c = Catalog::load(None).unwrap();
        let req = BundleRequest::from_json(&serde_json::json!({
            "optional": ["connector-screws"],
            "title": "Headphone hook",
            "brief": "Clamps under a 25 mm desk top and holds two pairs.",
            "printer": {"name": "Bambu Lab P1S", "bed": [256, 256, 256], "nozzle": 0.4},
            "photos": [
                {"name": "IMG_2041.JPG", "caption": "desk edge with a ruler", "data": "data:image/jpeg;base64,/9j/4AAQ"},
                {"name": "hook.png", "data": "iVBORw0KGgo="},
                {"name": "converted.png", "data": "/9j/4AAQ"}
            ]
        }))
        .unwrap();
        assert_eq!(req.photos[0].bytes, [0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10]);
        assert_eq!(req.photos[1].bytes[..4], [0x89, b'P', b'N', b'G']);

        let (name, zip) = c.bundle_zip(&req).unwrap();
        assert_eq!(name, "parametric-design-headphone-hook");
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
        let names: Vec<String> = z
            .file_names()
            .filter_map(|n| n.ok().map(|n| n.into_owned()))
            .collect();
        for want in [
            "parametric-design-headphone-hook/SKILL.md",
            "parametric-design-headphone-hook/brief/photo-1.jpg",
            "parametric-design-headphone-hook/brief/photo-2.png",
            "parametric-design-headphone-hook/brief/photo-3.jpg",
            "parametric-design-headphone-hook/references/core-parametric/components.md",
        ] {
            assert!(
                names.iter().any(|n| n == want),
                "{want} missing from {names:?}"
            );
        }
        let mut md = String::new();
        std::io::Read::read_to_string(
            &mut z
                .by_name("parametric-design-headphone-hook/SKILL.md")
                .unwrap(),
            &mut md,
        )
        .unwrap();
        assert!(md.contains("**What to design:** Headphone hook"));
        assert!(md.contains("Clamps under a 25 mm desk top"));
        assert!(md.contains("- Photo 1 (`brief/photo-1.jpg`): desk edge with a ruler"));
        assert!(md.contains("- Photo 2 (`brief/photo-2.png`)\n"));
        assert!(md.contains("Bambu Lab P1S (bed 256 × 256 × 256 mm, 0.4 mm nozzle)"));
        assert!(md.contains("Screws, nuts and heat-set inserts"));

        let prompt = c.prompt(&req).unwrap();
        assert!(prompt.contains("- Photo 1 (attached to this chat): desk edge with a ruler"));
        assert!(prompt.contains("# Reference files"));

        let s = skill_starter(&name, &req);
        assert!(s.contains("parametric-design-headphone-hook skill"));
        assert!(s.contains("attached the 3 photos"));
        assert!(prompt_starter("hook.md", &req).contains("attached hook.md"));
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("pd-skills-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn builtin_skills_have_catalog_entries() {
        let c = Catalog::builtin();
        assert!(c.problems.is_empty(), "{:?}", c.problems);
        assert_eq!(c.skills[0].id, "core-parametric");
        let dove = c.get("connector-dovetail").unwrap();
        assert_eq!(dove.category, "Connectors");
        assert_eq!(dove.requires, ["multi-part"]);
        // app-only files stay out of what the LLM gets
        let files = c.skill_files("connector-dovetail").unwrap();
        assert!(files.contains_key("SKILL.md") && !files.contains_key("skill.json"));
    }

    #[test]
    fn a_folder_is_a_skill_and_broken_ones_are_reported() {
        let dir = temp_dir("folder");
        std::fs::create_dir_all(dir.join("hinge/images")).unwrap();
        std::fs::write(
            dir.join("hinge/SKILL.md"),
            "---\nname: hinge\ndescription: Print-in-place hinges.\n---\n\n# Hinges\n\n![a](images/hinge.png)\n",
        )
        .unwrap();
        std::fs::write(dir.join("hinge/thumbnail.png"), b"\x89PNG....").unwrap();
        std::fs::write(dir.join("hinge/images/hinge.png"), b"\x89PNG").unwrap();
        std::fs::create_dir_all(dir.join("broken")).unwrap();
        std::fs::write(dir.join("broken/SKILL.md"), "---\nname: broken\n---\n").unwrap();
        std::fs::write(dir.join("broken/skill.json"), "{ nope").unwrap();

        let c = Catalog::installed(&dir).unwrap();
        let h = c.get("hinge").unwrap();
        assert_eq!((h.title.as_str(), h.category.as_str()), ("Hinge", "Other"));
        assert_eq!(h.summary, "Print-in-place hinges.");
        assert_eq!(h.thumbnail.as_deref(), Some("thumbnail.png"));
        assert!(c
            .thumbnail_data_url("hinge")
            .unwrap()
            .starts_with("data:image/png;base64,iVBORy4u"));
        assert!(c.get("broken").is_none());
        assert!(
            c.problems
                .iter()
                .any(|p| p.starts_with("broken: skill.json")),
            "{:?}",
            c.problems
        );
        // the built-in skills were copied in next to it
        assert!(c.get("core-parametric").is_some());
        let zip = c
            .bundle_files(&BundleRequest {
                optional: vec!["hinge".into()],
                ..Default::default()
            })
            .unwrap()
            .1;
        assert!(zip.contains_key("images/hinge/hinge.png"));
        assert!(!zip.keys().any(|k| k.contains("thumbnail")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sync_keeps_edits_and_deletions() {
        let dir = temp_dir("sync");
        assert!(sync_builtin(&dir, false).unwrap() > 0);
        assert_eq!(sync_builtin(&dir, false).unwrap(), 0);
        let edited = dir.join("printability/SKILL.md");
        std::fs::write(&edited, "---\nname: printability\n---\nmine").unwrap();
        std::fs::remove_dir_all(dir.join("connector-magnet")).unwrap();
        assert_eq!(sync_builtin(&dir, false).unwrap(), 0);
        assert!(std::fs::read_to_string(&edited).unwrap().ends_with("mine"));
        assert!(!dir.join("connector-magnet").exists());
        // an untouched file that is out of date is updated
        let old = dir.join("multi-part/SKILL.md");
        std::fs::write(&old, "old").unwrap();
        let mut record: BTreeMap<String, String> =
            serde_json::from_slice(&std::fs::read(dir.join(SYNC_FILE)).unwrap()).unwrap();
        record.insert("multi-part/SKILL.md".into(), fingerprint(b"old"));
        std::fs::write(dir.join(SYNC_FILE), serde_json::to_string(&record).unwrap()).unwrap();
        assert_eq!(sync_builtin(&dir, false).unwrap(), 1);
        assert_ne!(std::fs::read_to_string(&old).unwrap(), "old");
        // restore puts everything back
        assert!(sync_builtin(&dir, true).unwrap() >= 2);
        assert!(!std::fs::read_to_string(&edited).unwrap().ends_with("mine"));
        assert!(dir.join("connector-magnet/SKILL.md").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn single_skill_zip_and_limits() {
        let c = Catalog::load(None).unwrap();
        let zip = c.skill_zip("printability").unwrap();
        let z = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
        assert!(z
            .file_names()
            .any(|n| n.is_ok_and(|n| n == "printability/SKILL.md")));
        assert!(c.skill_zip("nope").is_err());
        let too_many = BundleRequest {
            photos: vec![Photo::default(); MAX_PHOTOS + 1],
            ..Default::default()
        };
        assert!(c.bundle_files(&too_many).is_err());
        assert!(unbase64("not base64!").is_err());
    }
}
