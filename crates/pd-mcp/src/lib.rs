//! Model Context Protocol server over stdio (newline-delimited JSON-RPC).
//!
//! Tools let an LLM check its own work before the user sees it:
//! validate -> render -> measure -> fix, then open the design in the app.

use pd_core::validate::{validate, ValidateOptions};
use pd_core::{design_from_any, overrides_from_json};
use pd_render::{render_png, Layout, RenderOptions, View};
use serde_json::{json, Value as Json};
use std::io::{BufRead, Write};
use std::path::PathBuf;

const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "ParametricDesign turns pd/0.1 JSON designs into printable 3D models. \
Workflow: (1) if the user says they prepared a request in the app (\"my request\", \"the request I prepared\"), call \
get_design_request: it holds their description, photos, printer and chosen skills; (2) if you have not loaded the \
ParametricDesign skill, call get_design_guide first (with the skills the request names); (3) if the user talks about \
the design they have open (\"this box\", \"make it taller\"), call get_open_design and change THAT design, keeping \
its params and ids; if it is not open, find it with list_designs and update it by its design_id instead of making a \
duplicate; (4) write the design; (5) call validate_design and fix every error; (6) call render_preview and \
look at the images to confirm the shape matches the request; use measure for fits and clearances; (7) when it is \
right, call open_in_app: mode \"update\" for a change to the open design (the user sees a side-by-side compare and \
accepts it), mode \"new\" for a different design (it is added to their library). \
Never show the user a design that has not passed validate_design.";

/// The printer chosen in the app's settings, used for bed-size checks.
fn user_printer() -> Option<pd_core::Printer> {
    pd_core::Printer::from_json(pd_library::config::printer().as_ref())
}

fn tools() -> Json {
    let design = json!({
        "description": "The pd/0.1 design as a JSON object (a JSON string is also accepted).",
        "type": ["object", "string"]
    });
    let params = json!({
        "type": "object",
        "description": "Optional param overrides, e.g. {\"width\": 120}. Defaults come from the design.",
        "additionalProperties": true
    });
    json!([
        {
            "name": "get_design_guide",
            "title": "Get the design guide",
            "description": "Returns the ParametricDesign skill text: the pd/0.1 format, every component, placement rules, printability rules and examples. Call this first if the ParametricDesign skill is not already loaded. Pass optional skill ids (e.g. multi-part, connector-dovetail, connector-snap-fit, electronics-enclosure) to include them.",
            "inputSchema": {"type": "object", "properties": {"skills": {"type": "array", "items": {"type": "string"}, "description": "Optional skills to include besides the core ones."}}},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "list_components",
            "title": "List components",
            "description": "Fields, defaults, anchors and an example for every component type (or one, with `type`) and every joint type.",
            "inputSchema": {"type": "object", "properties": {"type": {"type": "string", "description": "Only this component or joint type."}}},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "validate_design",
            "title": "Validate a design",
            "description": "Parses and builds the design at its defaults and at the min/max of every param, then checks printability. Returns VALID or a list of errors and warnings, each with a JSON path and a fix. Fix all errors and validate again before showing anything to the user.",
            "inputSchema": {"type": "object", "properties": {"design": design, "params": params}, "required": ["design"]},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "render_preview",
            "title": "Render preview images",
            "description": "Renders the design to a labelled PNG (default views: iso, front, right, top; all at one scale, with axes and overall dimensions). Look at it to check that the shape is what was asked for. Use layout 'exploded' to see separate parts, 'print' to see them as printed, and section to look inside.",
            "inputSchema": {"type": "object", "properties": {
                "design": design, "params": params,
                "views": {"type": "array", "items": {"type": "string", "enum": ["iso", "iso_back", "front", "back", "left", "right", "top", "bottom"]}},
                "layout": {"type": "string", "enum": ["assembled", "exploded", "print"]},
                "section": {"type": "object", "properties": {"axis": {"type": "string", "enum": ["x", "y", "z"]}, "at": {"type": "number"}}, "description": "Cut the model at the plane axis = at to look inside; the cut faces are red. Front view shows y cuts, right view x cuts, top view z cuts."},
                "highlight": {"type": "string", "description": "Part id to show in colour; others are greyed."}
            }, "required": ["design"]},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "measure",
            "title": "Measure",
            "description": "Measurements of the built design. Queries: {type:'bbox', part} | {type:'volume', part} | {type:'distance', a, b} (gap or overlap between parts) | {type:'anchor', ref:'part.anchor'} | {type:'thickness', part} (thinnest wall) | {type:'section', part, axis, at} | {type:'params'}. With no queries: every part's box and the gaps between parts.",
            "inputSchema": {"type": "object", "properties": {"design": design, "params": params, "queries": {"type": "array", "items": {"type": "object"}}}, "required": ["design"]},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "get_open_design",
            "title": "Get the design open in the app",
            "description": "Returns the design the user has open in the ParametricDesign app right now: its id, name, version, the slider values they are using and the full design JSON. Call this before changing \"this design\"; then send the change back with open_in_app mode \"update\".",
            "inputSchema": {"type": "object", "properties": {}},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "list_designs",
            "title": "Find designs in the library",
            "description": "Lists the designs in the user's ParametricDesign library, newest change first: design_id, name, category, current version and when it last changed. Pass query to search names, categories and tags. Use it when the user means a design that is not open in the app; change it with open_in_app mode \"update\" and its design_id rather than creating a new design with the same name.",
            "inputSchema": {"type": "object", "properties": {"query": {"type": "string", "description": "Words from the design's name, category or tags."}}},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "get_design_request",
            "title": "Get the design request prepared in the app",
            "description": "Returns the newest design request the user prepared in the ParametricDesign app (New design -> Design with AI): their description, reference photos (as images), their printer and the optional skills they picked. Call this when the user says they prepared or sent a request in the app, then design it and send it with open_in_app mode \"new\".",
            "inputSchema": {"type": "object", "properties": {}},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "open_in_app",
            "title": "Send to ParametricDesign",
            "description": "Sends a validated design to the ParametricDesign app. mode \"new\" (default): adds it to the user's library as a new design. mode \"update\": proposes it as a new version of an existing design (design_id from get_open_design or list_designs); the app shows the old and new side by side and the user accepts or rejects it. Give a short message saying what changed. Only call this after validate_design passes.",
            "inputSchema": {"type": "object", "properties": {
                "design": design,
                "mode": {"type": "string", "enum": ["new", "update"], "description": "new = another design; update = change to an existing design."},
                "design_id": {"type": "string", "description": "For mode update: the id from get_open_design or list_designs. Defaults to the design open in the app."},
                "message": {"type": "string", "description": "What changed or what this is, in one sentence (shown to the user)."}
            }, "required": ["design"]}
        },
        {
            "name": "export_design",
            "title": "Export STL / 3MF",
            "description": "Builds the design and writes a print file (stl or 3mf) to the ParametricDesign exports folder, or to `path`. 3MF keeps parts, colours and the editable design.",
            "inputSchema": {"type": "object", "properties": {"design": design, "params": params, "format": {"type": "string", "enum": ["stl", "3mf"]}, "path": {"type": "string"}}, "required": ["design", "format"]}
        }
    ])
}

fn text(t: impl Into<String>) -> Json {
    json!({"type": "text", "text": t.into()})
}

fn tool_error(msg: impl Into<String>) -> Json {
    json!({"content": [text(msg)], "isError": true})
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

/// Validate quickly (no extremes) and return the model, or a tool error.
fn built(
    args: &Json,
) -> Result<
    (
        Json,
        pd_core::Design,
        pd_core::Model,
        pd_core::validate::Report,
    ),
    Json,
> {
    let raw = args
        .get("design")
        .ok_or_else(|| tool_error("missing \"design\""))?;
    let design_json = design_from_any(raw).map_err(|e| tool_error(e.to_line()))?;
    let overrides = overrides_from_json(args.get("params"));
    let (report, design, model) = validate(
        &design_json,
        &ValidateOptions {
            overrides,
            printer: user_printer(),
            extremes: false,
            thorough: false,
        },
    );
    match (design, model) {
        (Some(d), Some(m)) if !m.has_errors() => Ok((design_json, d, m, report)),
        _ => Err(tool_error(format!(
            "The design does not build, so there is nothing to show yet.\n\n{}",
            report.to_text()
        ))),
    }
}

/// "5 minutes ago", "yesterday"...
fn ago(ms: u64) -> String {
    let mins = pd_library::now_ms().saturating_sub(ms) / 60_000;
    match mins {
        0 => "just now".into(),
        1..=59 => format!("{mins} minutes ago"),
        60..=1439 => format!("{} hours ago", mins / 60),
        1440..=2879 => "yesterday".into(),
        _ => format!("{} days ago", mins / 1440),
    }
}

fn num(v: Option<&Json>) -> String {
    let s = format!("{:.1}", v.and_then(|x| x.as_f64()).unwrap_or(0.0));
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The request as text (brief, photo notes, printer, next steps) followed by
/// the photos as images.
fn design_request_content(dir: &std::path::Path, req: &Json) -> Vec<Json> {
    let field = |k: &str| {
        req.get(k)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    let mut out = String::from("Design request the user prepared in the ParametricDesign app.\n\n");
    if let Some(t) = field("title") {
        out.push_str(&format!("What to design: {t}\n\n"));
    }
    if let Some(b) = field("brief") {
        out.push_str(b);
        out.push_str("\n\n");
    }
    let mut images = vec![];
    for p in req
        .get("photos")
        .and_then(|p| p.as_array())
        .into_iter()
        .flatten()
    {
        let file = p.get("file").and_then(|f| f.as_str()).unwrap_or_default();
        let Ok(bytes) = std::fs::read(dir.join(file)) else {
            continue;
        };
        let mime = match file.rsplit('.').next() {
            Some("png") => "image/png",
            Some("webp") => "image/webp",
            Some("gif") => "image/gif",
            _ => "image/jpeg",
        };
        images.push(json!({"type": "image", "data": base64(&bytes), "mimeType": mime}));
        let caption = p
            .get("caption")
            .and_then(|c| c.as_str())
            .map(str::trim)
            .filter(|c| !c.is_empty());
        out.push_str(&format!(
            "- Photo {} (image {} below){}\n",
            images.len(),
            images.len(),
            caption.map(|c| format!(": {c}")).unwrap_or_default()
        ));
    }
    if let Some(p) = req.get("printer").filter(|p| p.get("bed").is_some()) {
        let bed = p.get("bed");
        out.push_str(&format!(
            "- Printer: {} (bed {} × {} × {} mm, {} mm nozzle). Every part must fit on this bed.\n",
            p.get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("their printer"),
            num(bed.and_then(|b| b.get(0))),
            num(bed.and_then(|b| b.get(1))),
            num(bed.and_then(|b| b.get(2))),
            num(p.get("nozzle"))
        ));
    }
    let skills: Vec<&str> = req
        .get("optional")
        .and_then(|o| o.as_array())
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str())
        .collect();
    out.push_str(&format!(
        "\nNext: unless you already loaded these skills, call get_design_guide{} (the core skills are always included). \
Then write the design, call validate_design and render_preview, fix what they show, and send it with open_in_app mode \"new\". \
Design exactly this; don't ask follow-up questions unless something essential is missing, and expose anything uncertain as a parameter.",
        if skills.is_empty() {
            String::new()
        } else {
            format!(" with skills {}", json!(skills))
        }
    ));
    let mut content = vec![text(out)];
    content.extend(images);
    content
}

fn catalog() -> Result<pd_skills::Catalog, Json> {
    pd_skills::Catalog::load(None).map_err(tool_error)
}

pub fn call_tool(name: &str, args: &Json) -> Json {
    let r: Result<Json, Json> = (|| match name {
        "get_design_guide" => {
            let skills: Vec<String> = args
                .get("skills")
                .and_then(|s| s.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let cat = catalog()?;
            let mut guide = cat
                .prompt(&pd_skills::BundleRequest {
                    optional: skills,
                    ..Default::default()
                })
                .map_err(tool_error)?;
            guide.push_str(
                "\n\nOptional skills you can request with get_design_guide(skills=[...]):\n",
            );
            for s in cat.skills.iter().filter(|s| !s.core) {
                guide.push_str(&format!("- {}: {}\n", s.id, s.summary));
            }
            Ok(json!({"content": [text(guide)]}))
        }
        "list_components" => {
            let v = pd_core::spec::components_json(args.get("type").and_then(|t| t.as_str()));
            Ok(json!({"content": [text(serde_json::to_string_pretty(&v).unwrap())]}))
        }
        "validate_design" => {
            let raw = args
                .get("design")
                .ok_or_else(|| tool_error("missing \"design\""))?;
            let design_json = match design_from_any(raw) {
                Ok(j) => j,
                Err(e) => {
                    return Ok(
                        json!({"content": [text(format!("NOT VALID: 1 error(s).\n  {}", e.to_line()))]}),
                    )
                }
            };
            let overrides = overrides_from_json(args.get("params"));
            let (report, _, _) = validate(
                &design_json,
                &ValidateOptions {
                    overrides,
                    printer: user_printer(),
                    extremes: true,
                    thorough: true,
                },
            );
            let mut t = report.to_text();
            if report.ok {
                t.push_str("\nNext: call render_preview and check the images match the request.");
            }
            Ok(
                json!({"content": [text(t)], "structuredContent": serde_json::to_value(&report).unwrap()}),
            )
        }
        "render_preview" => {
            let (_, _, model, report) = built(args)?;
            let mut o = RenderOptions::default();
            if let Some(vs) = args.get("views").and_then(|v| v.as_array()) {
                let views: Option<Vec<View>> = vs
                    .iter()
                    .map(|v| v.as_str().and_then(View::parse))
                    .collect();
                o.views = views.ok_or_else(|| tool_error("unknown view name"))?;
            }
            o.layout = match args.get("layout").and_then(|l| l.as_str()) {
                Some("exploded") => Layout::Exploded,
                Some("print") => Layout::Print,
                _ => Layout::Assembled,
            };
            if let Some(s) = args.get("section") {
                let axis = s
                    .get("axis")
                    .and_then(|a| a.as_str())
                    .and_then(|a| a.chars().next())
                    .unwrap_or('y');
                o.section = Some((axis, s.get("at").and_then(|a| a.as_f64()).unwrap_or(0.0)));
            }
            o.highlight = args
                .get("highlight")
                .and_then(|h| h.as_str())
                .map(String::from);
            let png = render_png(&model, &o);
            let mut summary = report.to_text();
            summary.push_str("\nCheck the images: is every part where it should be, the right size and the right way up? Red faces are section cuts.");
            Ok(
                json!({"content": [{"type": "image", "data": base64(&png), "mimeType": "image/png"}, text(summary)]}),
            )
        }
        "measure" => {
            let (_, _, model, _) = built(args)?;
            let queries: Vec<Json> = args
                .get("queries")
                .and_then(|q| q.as_array())
                .cloned()
                .unwrap_or_default();
            let res = pd_core::measure::measure(&model, &queries);
            Ok(json!({"content": [text(serde_json::to_string_pretty(&res).unwrap())]}))
        }
        "get_open_design" => {
            let Some(cur) = pd_library::config::read_current() else {
                return Ok(
                    json!({"content": [text("The ParametricDesign app has not reported an open design. Ask the user to open the design in the app, or create a new one with open_in_app mode \"new\".")]}),
                );
            };
            if !cur.get("open").and_then(|o| o.as_bool()).unwrap_or(false) {
                return Ok(
                    json!({"content": [text("No design is open in the ParametricDesign app right now (the user is in the library). If they mean a design they already have, find it with list_designs and send the change with open_in_app mode \"update\" and its design_id; use mode \"new\" only for a different object.")]}),
                );
            }
            let summary = format!(
                "Open in the app: \"{}\" (design_id {}, version v{}{}). The params object holds the slider values the user is looking at; keep the param names and part ids when you change it, and send it back with open_in_app mode \"update\".",
                cur.get("name").and_then(|n| n.as_str()).unwrap_or("?"),
                cur.get("designId").and_then(|n| n.as_str()).unwrap_or("?"),
                cur.get("version").and_then(|n| n.as_u64()).unwrap_or(0),
                if cur.get("unsaved").and_then(|u| u.as_bool()).unwrap_or(false) { ", with unsaved slider changes" } else { "" }
            );
            Ok(
                json!({"content": [text(summary), text(serde_json::to_string_pretty(&json!({
                "designId": cur.get("designId"), "name": cur.get("name"), "version": cur.get("version"),
                "params": cur.get("params"), "design": cur.get("design")
            })).unwrap())]}),
            )
        }
        "list_designs" => {
            let Some(root) = pd_library::config::library_path() else {
                return Ok(
                    json!({"content": [text("The ParametricDesign app has no library folder yet. Ask the user to open the app once and choose one.")]}),
                );
            };
            let lib = pd_library::Library::open(root).map_err(tool_error)?;
            let q = args
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("")
                .trim()
                .to_lowercase();
            let rows: Vec<Json> = lib
                .metas()
                .into_iter()
                .filter(|m| m.trashed.is_none())
                .filter(|m| {
                    q.is_empty()
                        || m.name.to_lowercase().contains(&q)
                        || m.category.to_lowercase().contains(&q)
                        || m.tags.iter().any(|t| t.to_lowercase().contains(&q))
                })
                .take(50)
                .map(|m| {
                    json!({"design_id": m.id, "name": m.name, "category": m.category,
                           "version": m.current_version, "changed": ago(m.modified)})
                })
                .collect();
            let head = if rows.is_empty() {
                "No designs match.".to_string()
            } else {
                format!(
                    "{} design(s). To change one, send it with open_in_app mode \"update\" and its design_id.",
                    rows.len()
                )
            };
            Ok(json!({"content": [text(head), text(serde_json::to_string_pretty(&rows).unwrap())]}))
        }
        "get_design_request" => {
            let Some((dir, req)) = pd_library::config::latest_request() else {
                return Ok(
                    json!({"content": [text("No design request has been prepared in the ParametricDesign app yet. Ask the user to describe the design here, or to prepare it in the app (New design -> Design with AI -> Send to Claude).")]}),
                );
            };
            Ok(json!({"content": design_request_content(&dir, &req)}))
        }
        "open_in_app" => {
            let raw = args
                .get("design")
                .ok_or_else(|| tool_error("missing \"design\""))?;
            let design_json = design_from_any(raw).map_err(|e| tool_error(e.to_line()))?;
            let (report, _, _) = validate(
                &design_json,
                &ValidateOptions {
                    printer: user_printer(),
                    extremes: false,
                    thorough: false,
                    ..Default::default()
                },
            );
            if !report.ok {
                return Err(tool_error(format!(
                    "Not sent: fix the errors first.\n\n{}",
                    report.to_text()
                )));
            }
            let mode = args.get("mode").and_then(|m| m.as_str()).unwrap_or("new");
            let message = args
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let item = if mode == "update" {
                let id = args
                    .get("design_id")
                    .and_then(|d| d.as_str())
                    .map(String::from)
                    .or_else(|| {
                        pd_library::config::read_current()
                            .filter(|c| c.get("open").and_then(|o| o.as_bool()).unwrap_or(false))
                            .and_then(|c| c.get("designId").and_then(|d| d.as_str()).map(String::from))
                    })
                    .ok_or_else(|| tool_error("mode \"update\" needs design_id (call get_open_design), or a design open in the app"))?;
                json!({"kind": "update", "designId": id, "design": design_json, "message": message})
            } else {
                json!({"kind": "new", "design": design_json, "message": message})
            };
            pd_library::config::write_inbox(&item).map_err(tool_error)?;
            let note = if mode == "update" {
                format!("Sent \"{}\" to the app as an update. The user now sees the current and the new version side by side and decides whether to accept it.", report.name)
            } else {
                format!("Sent \"{}\" to the app; it is added to the user's library (marked new from Claude). If the app is not running, it appears the next time it starts.", report.name)
            };
            Ok(json!({"content": [text(note)]}))
        }
        "export_design" => {
            let (design_json, design, model, _) = built(args)?;
            let fmt = args
                .get("format")
                .and_then(|f| f.as_str())
                .unwrap_or("3mf")
                .to_ascii_lowercase();
            let bytes = match fmt.as_str() {
                "stl" => {
                    let parts: Vec<&pd_core::BuiltPart> = model.parts.iter().collect();
                    pd_export::stl(&parts, pd_export::Pose::Print)
                }
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
                    pd_export::three_mf(
                        &model,
                        &pd_export::ThreeMfOptions {
                            title: design.name.clone(),
                            bed: design.print.bed.unwrap_or([256.0, 256.0, 256.0]),
                            design: Some(design_json),
                            params: Some(Json::Object(params)),
                            thumbnail_png: Some(thumb),
                        },
                    )
                    .map_err(tool_error)?
                }
                other => {
                    return Err(tool_error(format!(
                        "unknown format \"{other}\"; use stl or 3mf"
                    )))
                }
            };
            let path = match args.get("path").and_then(|p| p.as_str()) {
                Some(p) => PathBuf::from(p),
                None => pd_library::config::exports_dir()
                    .join(format!("{}.{fmt}", pd_library::slug(&design.name))),
            };
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| tool_error(e.to_string()))?;
            }
            std::fs::write(&path, bytes)
                .map_err(|e| tool_error(format!("cannot write {}: {e}", path.display())))?;
            Ok(json!({"content": [text(format!("Wrote {}", path.display()))]}))
        }
        other => Err(tool_error(format!("unknown tool \"{other}\""))),
    })();
    r.unwrap_or_else(|e| e)
}

fn resources() -> Vec<(String, String, String)> {
    let mut out = vec![(
        "pd://reference/components.md".to_string(),
        "Component reference".to_string(),
        "text/markdown".to_string(),
    )];
    if let Ok(cat) = pd_skills::Catalog::load(None) {
        for s in &cat.skills {
            out.push((
                format!("pd://skills/{}/SKILL.md", s.id),
                format!("Skill: {}", s.title),
                "text/markdown".into(),
            ));
        }
    }
    out
}

fn read_resource(uri: &str) -> Option<String> {
    if uri == "pd://reference/components.md" {
        return Some(pd_core::spec::components_markdown());
    }
    let rest = uri.strip_prefix("pd://skills/")?;
    pd_skills::Catalog::load(None).ok()?.text(rest)
}

/// Handle one JSON-RPC message. Returns the response (None for notifications).
pub fn handle(msg: &Json) -> Option<Json> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    let id = id?;
    let result: Result<Json, (i64, String)> = match method {
        "initialize" => {
            let asked = params
                .get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let version = if PROTOCOL_VERSIONS.contains(&asked) {
                asked
            } else {
                PROTOCOL_VERSIONS[1]
            };
            Ok(json!({
                "protocolVersion": version,
                "capabilities": {"tools": {"listChanged": false}, "resources": {"listChanged": false}, "prompts": {"listChanged": false}},
                "serverInfo": {"name": "parametricdesign", "title": "ParametricDesign", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => {
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            Ok(call_tool(name, &args))
        }
        "resources/list" => Ok(
            json!({"resources": resources().into_iter().map(|(uri, name, mime)| json!({"uri": uri, "name": name, "mimeType": mime})).collect::<Vec<_>>()}),
        ),
        "resources/read" => {
            let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            match read_resource(uri) {
                Some(t) => {
                    Ok(json!({"contents": [{"uri": uri, "mimeType": "text/markdown", "text": t}]}))
                }
                None => Err((-32002, format!("resource not found: {uri}"))),
            }
        }
        "prompts/list" => Ok(json!({"prompts": [{
            "name": "design",
            "title": "Design a parametric part",
            "description": "Start a ParametricDesign session with the design guide loaded.",
            "arguments": [
                {"name": "brief", "description": "What to design, with sizes if known", "required": true},
                {"name": "skills", "description": "Optional skills, comma separated (e.g. multi-part,connector-dovetail)", "required": false}
            ]
        }]})),
        "prompts/get" => {
            let a = params.get("arguments").cloned().unwrap_or(json!({}));
            let brief = a
                .get("brief")
                .and_then(|b| b.as_str())
                .unwrap_or("")
                .to_string();
            let skills: Vec<String> = a
                .get("skills")
                .and_then(|s| s.as_str())
                .map(|s| {
                    s.split(',')
                        .map(|x| x.trim().to_string())
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            match pd_skills::Catalog::load(None).and_then(|c| {
                c.prompt(&pd_skills::BundleRequest {
                    optional: skills,
                    brief: Some(brief),
                    ..Default::default()
                })
            }) {
                Ok(p) => Ok(
                    json!({"description": "ParametricDesign session", "messages": [{"role": "user", "content": {"type": "text", "text": p}}]}),
                ),
                Err(e) => Err((-32602, e)),
            }
        }
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    Some(match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
        Err((code, m)) => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": m}})
        }
    })
}

pub fn serve_stdio() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let resp = match serde_json::from_str::<Json>(&line) {
            Ok(Json::Array(batch)) => {
                let rs: Vec<Json> = batch.iter().filter_map(handle).collect();
                if rs.is_empty() {
                    None
                } else {
                    Some(Json::Array(rs))
                }
            }
            Ok(msg) => handle(&msg),
            Err(e) => Some(
                json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}}),
            ),
        };
        if let Some(r) = resp {
            writeln!(out, "{}", serde_json::to_string(&r).unwrap())?;
            out.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn initialize_and_list() {
        let r = handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}})).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert!(
            handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none()
        );
        let r = handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).unwrap();
        assert_eq!(r["result"]["tools"].as_array().unwrap().len(), 10);
    }

    #[test]
    fn validate_and_render_tools() {
        let d = json!({"format": "pd/0.1", "meta": {"name": "t"}, "parts": [{"id": "a", "shape": {"type": "box", "size": [10, 10, 10]}}]});
        let r = call_tool("validate_design", &json!({"design": d}));
        assert!(r["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("VALID"));
        let r = call_tool("render_preview", &json!({"design": d.to_string()}));
        assert_eq!(r["content"][0]["type"], "image");
        let bad = json!({"format": "pd/0.1", "parts": [{"id": "a", "shape": {"type": "cube"}}]});
        let r = call_tool("render_preview", &json!({"design": bad}));
        assert_eq!(r["isError"], true);
    }

    #[test]
    fn open_design_and_update_flow() {
        let home = std::env::temp_dir().join(format!("pd-mcp-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::env::set_var("PD_HOME", &home);
        let d = json!({"format": "pd/0.1", "meta": {"name": "Cup"}, "parts": [{"id": "a", "shape": {"type": "cylinder", "d": 60, "h": 80}}]});
        let r = call_tool("get_open_design", &json!({}));
        assert!(r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not reported"));
        pd_library::config::write_current(&json!({"open": true, "designId": "cup-1", "name": "Cup", "version": 3, "params": {}, "design": d})).unwrap();
        let r = call_tool("get_open_design", &json!({}));
        assert!(r["content"][0]["text"].as_str().unwrap().contains("cup-1"));
        let r = call_tool(
            "open_in_app",
            &json!({"design": d, "mode": "update", "message": "taller"}),
        );
        assert!(r.get("isError").is_none(), "{r}");
        let files: Vec<_> = std::fs::read_dir(home.join("inbox"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(files.len(), 1);
        let item: Json =
            serde_json::from_str(&std::fs::read_to_string(files[0].path()).unwrap()).unwrap();
        assert_eq!(item["kind"], "update");
        assert_eq!(item["designId"], "cup-1");
        assert_eq!(item["message"], "taller");

        // a design in the library that is not open: found by name, updated by id
        let lib_dir = home.join("library");
        pd_library::config::save_settings(&json!({"libraryPath": lib_dir})).unwrap();
        let lib = pd_library::Library::open(&lib_dir).unwrap();
        let spacer = lib
            .create(&json!({"format": "pd/0.1", "meta": {"name": "Wall spacer", "tags": ["furniture"]}, "parts": [{"id": "a", "shape": {"type": "box", "size": [20, 20, 100]}}]}), &json!({}), "claude", "First", true)
            .unwrap();
        pd_library::config::write_current(&json!({"open": false})).unwrap();
        let r = call_tool("list_designs", &json!({"query": "furni"}));
        assert!(r["content"][1]["text"].as_str().unwrap().contains(&spacer));
        let r = call_tool("list_designs", &json!({"query": "nothing like it"}));
        assert_eq!(r["content"][0]["text"], "No designs match.");
        let r = call_tool(
            "open_in_app",
            &json!({"design": d, "mode": "update", "design_id": spacer, "message": "longer"}),
        );
        assert!(r.get("isError").is_none(), "{r}");
        let ev = lib.process_inbox(&home.join("inbox"));
        assert_eq!(ev.len(), 2, "the earlier update and this one");
        assert!(ev.iter().any(|e| e.kind == "update" && e.id == spacer));

        // a request prepared in the app (same test: PD_HOME is process-wide)
        let r = call_tool("get_design_request", &json!({}));
        assert!(r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("No design request"));
        pd_library::config::write_request(&json!({
            "title": "Headphone hook",
            "brief": "Clamps under a 25 mm desk top.",
            "optional": ["connector-screws"],
            "printer": {"name": "Bambu Lab P1S", "bed": [256, 256, 256], "nozzle": 0.4},
            "photos": [{"name": "desk.jpg", "caption": "desk edge with a ruler", "data": "data:image/jpeg;base64,/9j/4AAQ"}]
        }))
        .unwrap();
        let r = call_tool("get_design_request", &json!({}));
        let t = r["content"][0]["text"].as_str().unwrap();
        assert!(t.contains("What to design: Headphone hook"), "{t}");
        assert!(t.contains("- Photo 1 (image 1 below): desk edge with a ruler"));
        assert!(t.contains("bed 256 × 256 × 256 mm, 0.4 mm nozzle"));
        assert!(t.contains("get_design_guide with skills [\"connector-screws\"]"));
        assert_eq!(r["content"][1]["type"], "image");
        assert_eq!(r["content"][1]["mimeType"], "image/jpeg");
        assert_eq!(r["content"][1]["data"], "/9j/4AAQ");
        let _ = std::fs::remove_dir_all(&home);
    }
}
