# Skills

Every folder here is one skill. The app and its connector read this folder each time they need it. To add a skill or change one, edit the files and the change is used on the next request. There's nothing to compile or restart.

Built-in skills start out in `Documents\ParametricDesign Skills`. Open it from **Design with AI → Open skills folder**.

## A skill folder

```
connector-dovetail/
  SKILL.md         for the LLM: the full instructions (required)
  skill.json       for the catalog: title, category, description, ... (optional)
  thumbnail.png    for the catalog: a picture of what it makes (optional)
  images/          for the LLM: labelled drawings the instructions refer to
  references/      for the LLM: longer reference pages
  examples/        for the LLM: finished designs that use the skill
```

The folder name is the skill's id (lowercase, with dashes).

### SKILL.md (for the LLM)

It starts with a short header, then the instructions in Markdown:

```markdown
---
name: connector-dovetail
description: Joins split pieces with sliding dovetail keys. Use when the user wants dovetail connectors.
---

# Dovetail connectors

How to build it step by step, which sizes matter, good defaults and
tolerances, which parameters to give the user, when not to use it,
and common mistakes.

![Dovetail joint with its dimensions](images/dovetail.png)
```

- `name` must match the folder name.
- Write it for the LLM. Be complete and exact, because the LLM only knows what the skill tells it.
- **Reference images help consistency.** Use labelled drawings with the same names as the parameters (`key_width`, `angle`, `clearance`), and link them from the text. Link other files with relative paths (`images/…`, `references/…`, `examples/…`).

### skill.json (for the user)

```json
{
  "title": "Connector: dovetail",
  "category": "Connectors",
  "summary": "Split pieces joined by sliding dovetail keys (flat or tray-like objects).",
  "order": 50,
  "group": "connector",
  "requires": ["multi-part"]
}
```

| Field | Meaning | If left out |
|---|---|---|
| `title` | Name shown in the catalog | Made from the folder name |
| `category` | Catalog section it's listed under | `Other` |
| `summary` | What it does, in plain words for the user | The `description` from SKILL.md |
| `thumbnail` | Picture file in the folder | `thumbnail.png` (or `.jpg` / `.webp`) if there is one |
| `order` | Position in the catalog, lowest first | 1000 (after the built-in skills) |
| `core` | `true`: always included, can't be unticked | `false` |
| `group` | Skills with the same group are alternatives; the user picks one | none |
| `requires` | Skills that are added automatically with this one | none |

If a skill has a mistake (for example `skill.json` isn't valid JSON), the app leaves it out and says what's wrong. All the other skills keep working.

## Built-in skills and app updates

When the app updates, it updates the built-in skills you haven't changed and adds new ones. **It never overwrites a file you edited, never brings back a skill you deleted, and never touches skills you added.** **Restore built-in skills** in Design with AI puts back the original built-in files.

In this repository, `skills/` is the built-in set. After adding or removing a file here, run `PD_REGEN=1 cargo test -p pd-skills` to update the list compiled into the app. To try changes from this folder without copying them, set `PD_SKILLS` to its path.
