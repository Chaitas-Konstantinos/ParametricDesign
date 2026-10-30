// The skills/ folder, embedded in the binary. A list instead of a build script,
// so building the app compiles and runs nothing extra. After adding or removing
// a file under skills/, regenerate it: PD_REGEN=1 cargo test -p pd-skills

#[rustfmt::skip]
pub static FILES: &[(&str, &[u8])] = &[
    ("catalog.json", include_bytes!("../../../skills/catalog.json")),
    ("connector-dovetail/SKILL.md", include_bytes!("../../../skills/connector-dovetail/SKILL.md")),
    ("connector-dovetail/examples/dovetail-tray.pd.json", include_bytes!("../../../skills/connector-dovetail/examples/dovetail-tray.pd.json")),
    ("connector-magnet/SKILL.md", include_bytes!("../../../skills/connector-magnet/SKILL.md")),
    ("connector-pin/SKILL.md", include_bytes!("../../../skills/connector-pin/SKILL.md")),
    ("connector-screws/SKILL.md", include_bytes!("../../../skills/connector-screws/SKILL.md")),
    ("connector-snap-fit/SKILL.md", include_bytes!("../../../skills/connector-snap-fit/SKILL.md")),
    ("connector-snap-fit/examples/snap-lid-box.pd.json", include_bytes!("../../../skills/connector-snap-fit/examples/snap-lid-box.pd.json")),
    ("core-parametric/SKILL.md", include_bytes!("../../../skills/core-parametric/SKILL.md")),
    ("core-parametric/examples/snap-lid-box.pd.json", include_bytes!("../../../skills/core-parametric/examples/snap-lid-box.pd.json")),
    ("core-parametric/examples/vase.pd.json", include_bytes!("../../../skills/core-parametric/examples/vase.pd.json")),
    ("core-parametric/examples/wall-bracket.pd.json", include_bytes!("../../../skills/core-parametric/examples/wall-bracket.pd.json")),
    ("core-parametric/references/format.md", include_bytes!("../../../skills/core-parametric/references/format.md")),
    ("electronics-enclosure/SKILL.md", include_bytes!("../../../skills/electronics-enclosure/SKILL.md")),
    ("electronics-enclosure/examples/board-enclosure.pd.json", include_bytes!("../../../skills/electronics-enclosure/examples/board-enclosure.pd.json")),
    ("from-photos/SKILL.md", include_bytes!("../../../skills/from-photos/SKILL.md")),
    ("multi-part/SKILL.md", include_bytes!("../../../skills/multi-part/SKILL.md")),
    ("printability/SKILL.md", include_bytes!("../../../skills/printability/SKILL.md")),
];
