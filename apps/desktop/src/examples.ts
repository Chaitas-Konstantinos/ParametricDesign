// The reference designs from /examples, bundled into the app.
const files = import.meta.glob("../../../examples/*.pd.json", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

const ORDER = ["snap-lid-box", "board-enclosure", "wall-bracket", "dovetail-tray", "vase"];

export const EXAMPLES = Object.entries(files)
  .map(([path, text]) => {
    const id = path.split("/").pop()!.replace(".pd.json", "");
    let name = id;
    try {
      name = JSON.parse(text).meta?.name ?? id;
    } catch {
      // keep the file name
    }
    return { id, name, text };
  })
  .sort((a, b) => (ORDER.indexOf(a.id) + 99) % 99 - (ORDER.indexOf(b.id) + 99) % 99);
