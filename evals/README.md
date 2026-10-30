# Evals

Measures how often an LLM, given the ParametricDesign skills, gets a design right.

- `cases.json`: 20 real requests (boxes, brackets, enclosures, split trays and vases, ...), each with the optional skills a user would pick and automatic checks (valid at every param extreme, part count, components used, joint type, overall size, fits the bed).
- `run.py`: runs the cases against the Claude API in two modes:
  - `--mode oneshot`: the tailored skill as the system prompt, no tools. Measures first-try success.
  - `--mode tools`: the same plus the `pd mcp` tools (validate, render, measure, open). Measures success after self-correction, and records the model's first attempt too.

```bash
pip install anthropic
cargo build --release -p pd-cli
python3 evals/run.py --mode oneshot
python3 evals/run.py --mode tools --cases long-tray,pi-case
```

Results go to `out/evals/<timestamp>-<mode>-<model>/results.json`, with every design the model produced. Milestone 1 targets: **80 % first try, 95 % after self-correction**. Each full run makes 20+ API calls; run a few cases first.
