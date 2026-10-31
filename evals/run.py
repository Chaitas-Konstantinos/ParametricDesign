#!/usr/bin/env python3
"""Eval harness: how often does an LLM, given our skills, produce a correct
ParametricDesign design on the first try, and after self-correcting with the
MCP tools?

    pip install anthropic
    cargo build --release -p pd-cli
    python3 evals/run.py --mode oneshot            # skill only, no tools
    python3 evals/run.py --mode tools              # skill + pd MCP tools (validate/render/measure)
    python3 evals/run.py --cases pen-cup,long-tray --model claude-opus-5-5

Credentials come from the environment (ANTHROPIC_API_KEY or an `ant auth login`
profile). Every run spends API credit: 20 cases x a few requests each.

Scoring per case:
  valid    the design passes `pd validate` (including every param at min/max)
  checks   case-specific checks from cases.json (part count, components,
           joint type, overall size, fits the bed)
  first    the first design the model produced (its first validate call, or
           its answer in oneshot mode)
  final    the design it ended with
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

import anthropic

ROOT = Path(__file__).resolve().parent.parent
# Evaluate the skills in this repository, not the ones in the user's skills folder.
os.environ.setdefault("PD_SKILLS", str(ROOT / "skills"))
DEFAULT_MODEL = "claude-opus-5-5"


def find_pd(arg):
    if arg:
        return arg
    for p in (ROOT / "target/release/pd", ROOT / "target/debug/pd"):
        if p.exists():
            return str(p)
    sys.exit("build the CLI first: cargo build --release -p pd-cli")


def skill_prompt(pd, skills):
    cmd = [pd, "skills", "bundle", "--prompt"]
    if skills:
        cmd += ["--with", ",".join(skills)]
    return subprocess.run(cmd, check=True, capture_output=True, text=True).stdout


def extract_design(text):
    """The last ```json block that looks like a design."""
    blocks = re.findall(r"```json\s*(.*?)```", text, re.S)
    for b in reversed(blocks):
        try:
            d = json.loads(b)
        except json.JSONDecodeError:
            continue
        if isinstance(d, dict) and "parts" in d:
            return d
    return None


def validate(pd, design, workdir, name):
    path = Path(workdir) / f"{name}.pd.json"
    path.write_text(json.dumps(design, indent=2))
    r = subprocess.run([pd, "validate", "--json", str(path)], capture_output=True, text=True)
    try:
        return json.loads(r.stdout)
    except json.JSONDecodeError:
        return {"ok": False, "errors": [{"message": r.stderr.strip() or "validator crashed"}], "warnings": [], "parts": []}


def shape_types(design):
    out = set()

    def walk(v):
        if isinstance(v, dict):
            if isinstance(v.get("type"), str) and "shape" not in v:
                out.add(v["type"])
            for x in v.values():
                walk(x)
        elif isinstance(v, list):
            for x in v:
                walk(x)

    walk(design.get("parts", []))
    return out


def score(design, report, checks):
    """Returns (passed, list of failed check descriptions)."""
    if design is None:
        return False, ["no design produced"]
    fails = []
    if not report.get("ok"):
        fails.append(f"invalid ({len(report.get('errors', []))} errors)")
    parts = report.get("parts", [])
    lo, hi = checks.get("parts", [1, 99])
    if not lo <= len(parts) <= hi:
        fails.append(f"{len(parts)} parts, expected {lo}-{hi}")
    missing = [c for c in checks.get("components", []) if c not in shape_types(design)]
    if missing:
        fails.append(f"missing components {missing}")
    if "joint" in checks:
        joints = [s.get("joint", {}).get("type") for s in design.get("splits", [])]
        if checks["joint"] not in joints:
            fails.append(f"no {checks['joint']} joint (got {joints})")
    if parts and "size" in checks:
        mn = [min(p["min"][i] for p in parts) for i in range(3)]
        mx = [max(p["max"][i] for p in parts) for i in range(3)]
        for axis, (a, b) in checks["size"].items():
            i = "xyz".index(axis)
            size = mx[i] - mn[i]
            if not a <= size <= b:
                fails.append(f"overall {axis} {size:.1f} not in {a}-{b}")
    if parts and "bed" in checks:
        bed = sorted(checks["bed"] + [999] * (3 - len(checks["bed"])))
        for p in parts:
            if any(s > b for s, b in zip(sorted(p["size"]), bed)):
                fails.append(f"part {p['id']} {p['size']} does not fit bed {checks['bed']}")
    return not fails, fails


class Mcp:
    """Minimal stdio MCP client for `pd mcp`."""

    def __init__(self, pd, home):
        env = dict(os.environ, PD_HOME=str(home))
        self.p = subprocess.Popen([pd, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=env)
        self.n = 0
        self.call("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "pd-evals", "version": "0"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.p.stdin.flush()

    def call(self, method, params):
        self.n += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.n, "method": method, "params": params}) + "\n")
        self.p.stdin.flush()
        resp = json.loads(self.p.stdout.readline())
        if "error" in resp:
            raise RuntimeError(resp["error"])
        return resp["result"]

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=10)


def to_api_content(mcp_content):
    """MCP tool result content -> Messages API tool_result content."""
    out = []
    for c in mcp_content:
        if c["type"] == "image":
            out.append({"type": "image", "source": {"type": "base64", "media_type": c["mimeType"], "data": c["data"]}})
        elif c["type"] == "text":
            out.append({"type": "text", "text": c["text"]})
    return out


def request(client, args, **kw):
    params = dict(model=args.model, max_tokens=64000, output_config={"effort": args.effort},
                  cache_control={"type": "ephemeral"}, **kw)  # the skill prompt repeats every turn
    if args.fallbacks:
        params.update(betas=["server-side-fallback-2026-07-01"], fallbacks="default")
    with client.beta.messages.stream(**params) as stream:
        msg = stream.get_final_message()
    if msg.stop_reason == "refusal":
        raise RuntimeError(f"refused: {msg.stop_details}")
    return msg


def text_of(msg):
    return "".join(b.text for b in msg.content if b.type == "text")


def run_oneshot(client, args, system, case):
    msg = request(client, args, system=system, messages=[{"role": "user", "content": case["request"] + "\n\nReply with the complete design."}])
    d = extract_design(text_of(msg))
    return d, d, 1


def run_tools(client, args, system, case, pd, workdir):
    mcp = Mcp(pd, Path(workdir) / "pd_home")
    try:
        tools = []
        for t in mcp.call("tools/list", {})["tools"]:
            tools.append({"name": t["name"], "description": t["description"], "input_schema": t["inputSchema"], "eager_input_streaming": True})
        messages = [{"role": "user", "content": case["request"] + "\n\nUse the ParametricDesign tools to validate and preview your design and fix it until it is right. Then call open_in_app and reply with the complete final design."}]
        first = final = None
        turns = 0
        for turns in range(1, args.max_turns + 1):
            msg = request(client, args, system=system, tools=tools, messages=messages)
            messages.append({"role": "assistant", "content": msg.content})
            if msg.stop_reason == "pause_turn":
                continue
            calls = [b for b in msg.content if b.type == "tool_use"]
            if not calls:
                final = extract_design(text_of(msg)) or final
                break
            results = []
            for c in calls:
                inp = c.input if isinstance(c.input, dict) else None
                if inp is None:
                    results.append({"type": "tool_result", "tool_use_id": c.id, "is_error": True, "content": "tool input was not a JSON object"})
                    continue
                if "design" in inp:
                    d = inp["design"]
                    if isinstance(d, str):
                        try:
                            d = json.loads(d)
                        except json.JSONDecodeError:
                            d = None
                    if isinstance(d, dict):
                        first = first or d
                        final = d
                r = mcp.call("tools/call", {"name": c.name, "arguments": inp})
                results.append({"type": "tool_result", "tool_use_id": c.id, "content": to_api_content(r["content"]), "is_error": bool(r.get("isError"))})
            messages.append({"role": "user", "content": results})
        return first, final, turns
    finally:
        mcp.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mode", choices=["oneshot", "tools"], default="oneshot")
    ap.add_argument("--model", default=DEFAULT_MODEL)
    ap.add_argument("--effort", default="high", choices=["low", "medium", "high", "xhigh", "max"])
    ap.add_argument("--cases", help="comma-separated case ids (default: all)")
    ap.add_argument("--max-turns", type=int, default=10)
    ap.add_argument("--pd", help="path to the pd binary")
    ap.add_argument("--out", default=str(ROOT / "out" / "evals"))
    ap.add_argument("--no-fallbacks", dest="fallbacks", action="store_false", help="disable server-side refusal fallbacks")
    args = ap.parse_args()

    pd = find_pd(args.pd)
    cases = json.loads((ROOT / "evals/cases.json").read_text())["cases"]
    if args.cases:
        wanted = set(args.cases.split(","))
        cases = [c for c in cases if c["id"] in wanted]
    run_dir = Path(args.out) / f"{time.strftime('%Y%m%d-%H%M%S')}-{args.mode}-{args.model}"
    run_dir.mkdir(parents=True, exist_ok=True)
    client = anthropic.Anthropic()

    rows = []
    for case in cases:
        wd = run_dir / case["id"]
        wd.mkdir()
        system = skill_prompt(pd, case.get("skills", []))
        t0 = time.time()
        try:
            if args.mode == "oneshot":
                first, final, turns = run_oneshot(client, args, system, case)
            else:
                first, final, turns = run_tools(client, args, system, case, pd, wd)
            error = None
        except (anthropic.APIError, RuntimeError) as e:
            first = final = None
            turns = 0
            error = str(e)
        row = {"id": case["id"], "turns": turns, "seconds": round(time.time() - t0), "error": error}
        for label, d in (("first", first), ("final", final)):
            rep = validate(pd, d, wd, label) if d else {"ok": False, "errors": [], "parts": []}
            ok, fails = score(d, rep, case.get("checks", {}))
            row[label] = {"valid": bool(rep.get("ok")), "pass": ok, "fails": fails, "errors": [e.get("message") for e in rep.get("errors", [])][:5]}
        rows.append(row)
        print(f"{case['id']:<20} first {'PASS' if row['first']['pass'] else 'fail'}  final {'PASS' if row['final']['pass'] else 'fail'}  "
              f"turns {turns:>2}  {'; '.join(row['final']['fails'] or row['first']['fails'])[:90]}", flush=True)

    n = len(rows)
    summary = {
        "model": args.model, "mode": args.mode, "effort": args.effort, "cases": n,
        "first_valid": sum(r["first"]["valid"] for r in rows) / n,
        "first_pass": sum(r["first"]["pass"] for r in rows) / n,
        "final_valid": sum(r["final"]["valid"] for r in rows) / n,
        "final_pass": sum(r["final"]["pass"] for r in rows) / n,
    }
    (run_dir / "results.json").write_text(json.dumps({"summary": summary, "rows": rows}, indent=2))
    print(json.dumps(summary, indent=2))
    print(f"details: {run_dir}")


if __name__ == "__main__":
    main()
