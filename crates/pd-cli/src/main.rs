//! `pd` — the ParametricDesign command line.

use pd_core::validate::{validate, ValidateOptions};
use pd_core::{Overrides, Value};
use pd_export::{stl, three_mf, Pose, ThreeMfOptions};
use pd_render::{render_png, Layout, RenderOptions, View};
use serde_json::Value as Json;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "pd — ParametricDesign command line

USAGE:
  pd validate <design.pd.json> [--set name=value]... [--json] [--quick]
  pd render   <design.pd.json> [-o out.png] [--views iso,front,right,top] [--layout assembled|exploded|print]
                               [--section y=0] [--highlight <part>] [--set name=value]...
  pd export   <design.pd.json> --format stl|3mf [-o out] [--part <id>] [--assembled] [--set name=value]...
  pd measure  <design.pd.json> [--query '{\"type\":\"distance\",\"a\":\"box\",\"b\":\"lid\"}']... [--set name=value]...
  pd docs components               Markdown reference of every component and joint
  pd skills list                   Skills in the catalog, by category
  pd skills folder                 Where the skills are read from (add or edit skills there)
  pd skills restore                Put back the original built-in skills in that folder
  pd skills export <id> [-o dir|file.zip]
                                   One skill as a folder (or a .zip)
  pd skills bundle [--with a,b] [--title name] [--brief text | --brief-file f] [--photo img]...
                   [-o dir|file.zip] [--prompt]
                                   Core skills + chosen optional skills merged into one tailored skill
  pd mcp                           Run the MCP server on stdio (for Claude Desktop, Cursor, LM Studio, ...)

Skills come from the skills folder (`pd skills folder`); --skills-dir <dir> or
PD_SKILLS=<dir> reads another folder as-is.

Values given with --set override params, e.g. --set width=120 --set style=round.";

struct Args {
    pos: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(raw: Vec<String>) -> Args {
        let takes_value = [
            "-o",
            "--set",
            "--views",
            "--layout",
            "--section",
            "--highlight",
            "--format",
            "--part",
            "--query",
            "--with",
            "--brief",
            "--brief-file",
            "--title",
            "--photo",
            "--skills-dir",
        ];
        let mut pos = vec![];
        let mut flags = vec![];
        let mut it = raw.into_iter();
        while let Some(a) = it.next() {
            if a.starts_with('-') && a.len() > 1 {
                if let Some((k, v)) = a
                    .split_once('=')
                    .filter(|(k, _)| k.starts_with("--") && takes_value.contains(k))
                {
                    flags.push((k.to_string(), Some(v.to_string())));
                } else if takes_value.contains(&a.as_str()) {
                    flags.push((a, it.next()));
                } else {
                    flags.push((a, None));
                }
            } else {
                pos.push(a);
            }
        }
        Args { pos, flags }
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.flags
            .iter()
            .rev()
            .find(|(f, _)| f == k)
            .and_then(|(_, v)| v.as_deref())
    }
    fn all(&self, k: &str) -> Vec<&str> {
        self.flags
            .iter()
            .filter(|(f, _)| f == k)
            .filter_map(|(_, v)| v.as_deref())
            .collect()
    }
    fn has(&self, k: &str) -> bool {
        self.flags.iter().any(|(f, _)| f == k)
    }
}

fn fail(msg: impl AsRef<str>) -> ExitCode {
    eprintln!("error: {}", msg.as_ref());
    ExitCode::from(2)
}

fn read_design(path: &str) -> Result<Json, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path} is not valid JSON: {e}"))
}

fn overrides(args: &Args) -> Overrides {
    let mut o = Overrides::new();
    for kv in args.all("--set") {
        if let Some((k, v)) = kv.split_once('=') {
            let val = match serde_json::from_str::<Json>(v) {
                Ok(j) => Value::from_json(&j).unwrap_or(Value::Str(v.to_string())),
                Err(_) => Value::Str(v.to_string()),
            };
            o.insert(k.trim().to_string(), val);
        }
    }
    o
}

fn default_out(design: &str, ext: &str) -> PathBuf {
    let p = Path::new(design);
    let stem = p
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("design")
        .trim_end_matches(".json")
        .trim_end_matches(".pd");
    p.with_file_name(format!("{stem}.{ext}"))
}

fn build_model(
    args: &Args,
    path: &str,
) -> Result<(Json, pd_core::Design, pd_core::Model, Overrides), String> {
    let json = read_design(path)?;
    let o = overrides(args);
    let (report, design, model) = validate(
        &json,
        &ValidateOptions {
            overrides: o.clone(),
            printer: None,
            extremes: false,
            thorough: false,
        },
    );
    match (design, model) {
        (Some(d), Some(m)) if !m.has_errors() => Ok((json, d, m, o)),
        _ => Err(format!("the design has errors:\n{}", report.to_text())),
    }
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() || raw[0] == "-h" || raw[0] == "--help" || raw[0] == "help" {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let cmd = raw[0].clone();
    let args = Args::parse(raw[1..].to_vec());
    match cmd.as_str() {
        "validate" => {
            let Some(path) = args.pos.first() else {
                return fail("validate needs a design file");
            };
            let json = match read_design(path) {
                Ok(j) => j,
                Err(e) => return fail(e),
            };
            let opts = ValidateOptions {
                overrides: overrides(&args),
                printer: None,
                extremes: !args.has("--quick"),
                thorough: true,
            };
            let (report, _, _) = validate(&json, &opts);
            if args.has("--json") {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                print!("{}", report.to_text());
            }
            if report.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        "render" => {
            let Some(path) = args.pos.first() else {
                return fail("render needs a design file");
            };
            let (_, _, model, _) = match build_model(&args, path) {
                Ok(x) => x,
                Err(e) => return fail(e),
            };
            let mut opts = RenderOptions::default();
            if let Some(v) = args.get("--views") {
                let views: Option<Vec<View>> =
                    v.split(',').map(|s| View::parse(s.trim())).collect();
                match views {
                    Some(vs) => opts.views = vs,
                    None => return fail(
                        "unknown view; use iso, iso_back, front, back, left, right, top, bottom",
                    ),
                }
            }
            opts.layout = match args.get("--layout") {
                Some("exploded") => Layout::Exploded,
                Some("print") => Layout::Print,
                Some("assembled") | None => Layout::Assembled,
                Some(other) => return fail(format!("unknown layout {other}")),
            };
            if let Some(s) = args.get("--section") {
                let Some((axis, at)) = s.split_once('=') else {
                    return fail("--section looks like y=0");
                };
                let Ok(at) = at.parse::<f64>() else {
                    return fail("--section value must be a number");
                };
                opts.section = Some((axis.chars().next().unwrap_or('y'), at));
            }
            opts.highlight = args.get("--highlight").map(|s| s.to_string());
            let png = render_png(&model, &opts);
            let out = args
                .get("-o")
                .map(PathBuf::from)
                .unwrap_or_else(|| default_out(path, "png"));
            if let Err(e) = std::fs::write(&out, png) {
                return fail(format!("cannot write {}: {e}", out.display()));
            }
            println!("wrote {}", out.display());
            ExitCode::SUCCESS
        }
        "export" => {
            let Some(path) = args.pos.first() else {
                return fail("export needs a design file");
            };
            let (json, design, model, _) = match build_model(&args, path) {
                Ok(x) => x,
                Err(e) => return fail(e),
            };
            let format = args.get("--format").unwrap_or("3mf").to_ascii_lowercase();
            let parts: Vec<&pd_core::BuiltPart> = match args.get("--part") {
                Some(id) => match model.part(id) {
                    Some(p) => vec![p],
                    None => return fail(format!("no part {id}")),
                },
                None => model.parts.iter().collect(),
            };
            let bytes = match format.as_str() {
                "stl" => stl(
                    &parts,
                    if args.has("--assembled") {
                        Pose::Assembled
                    } else {
                        Pose::Print
                    },
                ),
                "3mf" => {
                    let thumb = render_png(
                        &model,
                        &RenderOptions {
                            views: vec![View::Iso],
                            cell: 256,
                            ..Default::default()
                        },
                    );
                    let params: serde_json::Map<String, Json> = model
                        .env
                        .iter()
                        .filter(|(k, _)| design.param(k).is_some())
                        .map(|(k, v)| (k.clone(), v.to_json()))
                        .collect();
                    match three_mf(
                        &model,
                        &ThreeMfOptions {
                            title: design.name.clone(),
                            bed: design.print.bed.unwrap_or([256.0, 256.0, 256.0]),
                            design: Some(json),
                            params: Some(Json::Object(params)),
                            thumbnail_png: Some(thumb),
                        },
                    ) {
                        Ok(b) => b,
                        Err(e) => return fail(e),
                    }
                }
                other => return fail(format!("unknown format {other}; use stl or 3mf")),
            };
            let out = args
                .get("-o")
                .map(PathBuf::from)
                .unwrap_or_else(|| default_out(path, &format));
            if let Err(e) = std::fs::write(&out, bytes) {
                return fail(format!("cannot write {}: {e}", out.display()));
            }
            println!("wrote {}", out.display());
            ExitCode::SUCCESS
        }
        "measure" => {
            let Some(path) = args.pos.first() else {
                return fail("measure needs a design file");
            };
            let (_, _, model, _) = match build_model(&args, path) {
                Ok(x) => x,
                Err(e) => return fail(e),
            };
            let queries: Result<Vec<Json>, _> = args
                .all("--query")
                .iter()
                .map(|q| serde_json::from_str::<Json>(q))
                .collect();
            let Ok(queries) = queries else {
                return fail("--query must be JSON");
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&pd_core::measure::measure(&model, &queries)).unwrap()
            );
            ExitCode::SUCCESS
        }
        "docs" => match args.pos.first().map(|s| s.as_str()) {
            Some("components") => {
                print!("{}", pd_core::spec::components_markdown());
                ExitCode::SUCCESS
            }
            _ => fail("usage: pd docs components"),
        },
        "skills" => skills(&args),
        "mcp" => match pd_mcp::serve_stdio() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(e.to_string()),
        },
        other => fail(format!("unknown command {other}\n\n{USAGE}")),
    }
}

fn is_zip(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

fn skills(args: &Args) -> ExitCode {
    let (dir, managed) = match args.get("--skills-dir") {
        Some(d) => (PathBuf::from(d), false),
        None => pd_library::config::skills_dir(),
    };
    match args.pos.first().map(|s| s.as_str()) {
        Some("folder") => {
            if managed {
                if let Err(e) = pd_skills::sync_builtin(&dir, false) {
                    return fail(e);
                }
            }
            println!("{}", dir.display());
            return ExitCode::SUCCESS;
        }
        Some("restore") => {
            return match pd_skills::sync_builtin(&dir, true) {
                Ok(n) => {
                    println!("restored {n} built-in files in {}", dir.display());
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            };
        }
        _ => {}
    }
    let loaded = if managed {
        pd_skills::Catalog::installed(&dir)
    } else {
        pd_skills::Catalog::load(Some(&dir))
    };
    let catalog = match loaded {
        Ok(c) => c,
        Err(e) => return fail(e),
    };
    for p in &catalog.problems {
        eprintln!("warning: {p}");
    }
    match args.pos.first().map(|s| s.as_str()) {
        Some("list") => {
            let mut category = "";
            for s in &catalog.skills {
                if s.category != category {
                    category = &s.category;
                    println!("{category}");
                }
                println!(
                    "  {:<22} {:<9} {}",
                    s.id,
                    if s.core { "core" } else { "optional" },
                    s.summary
                );
            }
            ExitCode::SUCCESS
        }
        Some("export") => {
            let Some(id) = args.pos.get(1) else {
                return fail("skills export needs a skill id");
            };
            let out = args
                .get("-o")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(id));
            let written = if is_zip(&out) {
                catalog
                    .skill_zip(id)
                    .and_then(|z| std::fs::write(&out, z).map_err(|e| e.to_string()))
            } else {
                catalog.export_one(id, &out)
            };
            match written {
                Ok(()) => {
                    println!("wrote {}", out.display());
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("bundle") => {
            let with: Vec<String> = args
                .get("--with")
                .map(|w| {
                    w.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let brief = match (args.get("--brief"), args.get("--brief-file")) {
                (Some(b), _) => Some(b.to_string()),
                (None, Some(f)) => match std::fs::read_to_string(f) {
                    Ok(s) => Some(s),
                    Err(e) => return fail(format!("cannot read {f}: {e}")),
                },
                _ => None,
            };
            let mut photos = vec![];
            for p in args.all("--photo") {
                match std::fs::read(p) {
                    Ok(bytes) => photos.push(pd_skills::Photo {
                        name: p.to_string(),
                        caption: None,
                        bytes,
                    }),
                    Err(e) => return fail(format!("cannot read {p}: {e}")),
                }
            }
            let req = pd_skills::BundleRequest {
                optional: with,
                title: args.get("--title").map(String::from),
                brief,
                photos,
                printer: None,
            };
            if args.has("--prompt") {
                match catalog.prompt(&req) {
                    Ok(p) => {
                        print!("{p}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => fail(e),
                }
            } else {
                let out = args
                    .get("-o")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("parametric-design-skill"));
                let written = if is_zip(&out) {
                    catalog.bundle_zip(&req).and_then(|(name, z)| {
                        std::fs::write(&out, z).map_err(|e| e.to_string())?;
                        Ok(name)
                    })
                } else {
                    catalog.bundle(&req, &out)
                };
                match written {
                    Ok(name) => {
                        println!("wrote tailored skill \"{name}\" to {}", out.display());
                        println!(
                            "\nOnce it is installed, start a chat with:\n{}",
                            pd_skills::skill_starter(&name, &req)
                        );
                        ExitCode::SUCCESS
                    }
                    Err(e) => fail(e),
                }
            }
        }
        _ => fail("usage: pd skills list | export <id> | bundle ..."),
    }
}
