//! The skill catalog. Core skills are always included; the user picks
//! optional ones. A selection can be exported as one tailored skill folder
//! (Agent Skills format), as a single pasteable prompt, or skill by skill.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillInfo {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub core: bool,
    /// Skills in the same group are alternatives (e.g. one connector type).
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    pub summary: String,
}

#[derive(Deserialize)]
struct CatalogFile {
    skills: Vec<SkillInfo>,
}

pub struct Catalog {
    pub skills: Vec<SkillInfo>,
    /// path relative to the skills root -> contents
    files: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Default)]
pub struct BundleRequest {
    /// Optional skill ids to add to the core skills.
    pub optional: Vec<String>,
    /// What the user wants, in their words.
    pub brief: Option<String>,
    /// Reference photos to include with the brief.
    pub photos: Vec<PathBuf>,
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

impl Catalog {
    /// Load from a skills directory, or the copy embedded in the binary.
    pub fn load(dir: Option<&Path>) -> Result<Catalog, String> {
        let mut files = BTreeMap::new();
        match dir {
            Some(d) => {
                fn walk(
                    dir: &Path,
                    root: &Path,
                    out: &mut BTreeMap<String, Vec<u8>>,
                ) -> std::io::Result<()> {
                    for e in std::fs::read_dir(dir)? {
                        let p = e?.path();
                        if p.is_dir() {
                            walk(&p, root, out)?;
                        } else {
                            out.insert(
                                p.strip_prefix(root)
                                    .unwrap()
                                    .to_string_lossy()
                                    .replace('\\', "/"),
                                std::fs::read(&p)?,
                            );
                        }
                    }
                    Ok(())
                }
                walk(d, d, &mut files)
                    .map_err(|e| format!("cannot read skills from {}: {e}", d.display()))?;
            }
            None => {
                for (p, b) in embedded::FILES {
                    files.insert(p.to_string(), b.to_vec());
                }
            }
        }
        // The component reference is always regenerated from the engine.
        files.insert(
            "core-parametric/references/components.md".into(),
            pd_core::spec::components_markdown().into_bytes(),
        );
        let cat: CatalogFile = serde_json::from_slice(
            files
                .get("catalog.json")
                .ok_or("skills/catalog.json is missing")?,
        )
        .map_err(|e| format!("bad catalog.json: {e}"))?;
        Ok(Catalog {
            skills: cat.skills,
            files,
        })
    }

    pub fn get(&self, id: &str) -> Option<&SkillInfo> {
        self.skills.iter().find(|s| s.id == id)
    }

    pub fn text(&self, path: &str) -> Option<String> {
        self.files
            .get(path)
            .map(|b| String::from_utf8_lossy(b).into_owned())
    }

    fn files_of(&self, id: &str) -> Vec<(&str, &Vec<u8>)> {
        let prefix = format!("{id}/");
        self.files
            .iter()
            .filter_map(|(p, b)| p.strip_prefix(&prefix).map(|r| (r, b)))
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

    /// Write one skill's folder as-is.
    pub fn export_one(&self, id: &str, out: &Path) -> Result<(), String> {
        if self.get(id).is_none() {
            return Err(format!("unknown skill \"{id}\""));
        }
        for (rel, bytes) in self.files_of(id) {
            let p = out.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn merged_body(
        &self,
        chosen: &[&SkillInfo],
        req: &BundleRequest,
        photo_names: &[String],
    ) -> String {
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
        if req.brief.is_some() || !photo_names.is_empty() {
            s.push_str("## The user's design brief\n\n");
            if let Some(b) = &req.brief {
                s.push_str(b.trim());
                s.push_str("\n\n");
            }
            for p in photo_names {
                s.push_str(&format!("- Reference photo: `brief/{p}`\n"));
            }
            if !photo_names.is_empty() {
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

    /// Write a single tailored skill folder. Returns its skill name.
    pub fn bundle(&self, req: &BundleRequest, out: &Path) -> Result<String, String> {
        let chosen = self.resolve(&req.optional)?;
        let extra: Vec<&str> = chosen
            .iter()
            .filter(|c| !c.core)
            .map(|c| c.title.as_str())
            .collect();
        let topic = req
            .brief
            .as_deref()
            .map(|b| b.lines().next().unwrap_or("").to_string())
            .filter(|b| !b.is_empty());
        let name = match &topic {
            Some(t) => format!("parametric-design-{}", slug(t)),
            None if !extra.is_empty() => format!("parametric-design-{}", slug(&extra.join(" "))),
            None => "parametric-design".to_string(),
        };
        let name: String = name
            .chars()
            .take(64)
            .collect::<String>()
            .trim_end_matches('-')
            .to_string();
        let mut desc = String::from("Writes ParametricDesign (pd/0.1) JSON designs: parametric, printable 3D models with sliders, built from components placed by anchors");
        if !extra.is_empty() {
            desc.push_str(&format!(", including {}", extra.join(", ").to_lowercase()));
        }
        desc.push_str(
            ". Use whenever the user wants a 3D-printable object designed, changed or fixed",
        );
        if let Some(t) = &topic {
            let short: String = t.chars().take(160).collect();
            desc.push_str(&format!(
                ", in particular: {}",
                short.replace(['\n', '"'], " ")
            ));
        }
        desc.push('.');
        let desc: String = desc.chars().take(1000).collect();

        std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
        let mut photo_names = vec![];
        if !req.photos.is_empty() {
            std::fs::create_dir_all(out.join("brief")).map_err(|e| e.to_string())?;
            for (i, p) in req.photos.iter().enumerate() {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
                let n = format!("photo-{}.{ext}", i + 1);
                std::fs::copy(p, out.join("brief").join(&n))
                    .map_err(|e| format!("cannot copy {}: {e}", p.display()))?;
                photo_names.push(n);
            }
        }
        let body = self.merged_body(&chosen, req, &photo_names);
        let skill_md = format!("---\nname: {name}\ndescription: {desc}\n---\n\n{body}");
        std::fs::write(out.join("SKILL.md"), skill_md).map_err(|e| e.to_string())?;
        for c in &chosen {
            for (rel, bytes) in self.files_of(&c.id) {
                if rel == "SKILL.md" {
                    continue;
                }
                let p = out.join(match rel.split_once('/') {
                    Some((dir, rest)) => format!("{dir}/{}/{rest}", c.id),
                    None => format!("references/{}/{rel}", c.id),
                });
                std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
                std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
            }
        }
        Ok(name)
    }

    /// Everything as one Markdown prompt (references inlined), for chat LLMs
    /// that cannot load skill folders.
    pub fn prompt(&self, req: &BundleRequest) -> Result<String, String> {
        let chosen = self.resolve(&req.optional)?;
        let photos: Vec<String> = req
            .photos
            .iter()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            })
            .collect();
        let mut s = self.merged_body(&chosen, req, &photos);
        if !photos.is_empty() {
            s = s.replace(
                "- Reference photo: `brief/",
                "- Reference photo (attached to this chat): `",
            );
        }
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

#[cfg(test)]
mod tests {
    use super::*;

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
                    photos: vec![],
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
}
