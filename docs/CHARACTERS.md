# Characters and the character creator

Every character is a **recipe of parts plus colours**. The creator, the inspector, scripts
and AI agents all build characters the same way, and old projects open unchanged.

## The creator

Open it from **Characters → ✎ Create a character**, or select a character and click
**✎ Open character creator** in the inspector.

- **Tabs**: Body, Hair, Face (eyes, eyebrows, nose, facial hair), Clothes, Wearables
  (headwear, eyewear, neckwear, earrings, on the back, held items) and My characters.
- **Part grid**: every option is drawn on the character you are building, so you see the
  real result. Face and head parts are shown close up.
- **Live preview**: plays idle, talk, wave, walk or cheer, with any expression.
- **Colours**: swatch rows for skin, hair and clothes, and for each wearable's own colours.
- **Randomise** gives a new character; tick **Keep** to protect tabs you like.
- **Undo and redo**: the buttons or Ctrl/⌘ Z, Ctrl/⌘ Shift Z.
- **My characters**: save any look and reuse it in any project (it is also listed in the
  Characters tab of the library). Start from the cast: pick Maya, Leo, Bea... to customise a copy.
- **Apply to <name>** changes the character on stage as one undo step; **Add to scene** adds a new one.

Cartoon and stick figures share the same parts where it makes sense: hair, body, facial hair
and all wearables fit both. Eye, eyebrow and nose styles are cartoon only; the dress is stick only.

## Part ids

| Category | Built-in parts |
|---|---|
| `body` | `body:slim`, `body:regular`, `body:broad` |
| `hair` | `hair:bald`, `hair:short`, `hair:spiky`, `hair:long`, `hair:bun`, `hair:curly` |
| `eyes` | `eyes:round`, `eyes:dots`, `eyes:wide`, `eyes:sleepy`, `eyes:lashes`, `eyes:narrow` |
| `brows` | `brows:normal`, `brows:thin`, `brows:thick`, `brows:arched`, `brows:none` |
| `nose` | `nose:button`, `nose:dot`, `nose:round`, `nose:long`, `nose:none` |
| `facialHair` | `facialHair:none`, `stubble`, `moustache`, `goatee`, `beard`, `fullBeard` |
| `outfit` | `outfit:clothes`, `outfit:dress` (stick) |
| `headwear` | `headwear:none`, `headwear:cap`, `headwear:bow`, plus pack parts |
| `eyewear` | `eyewear:none`, `eyewear:glasses`, plus pack parts |
| `neckwear` | `neckwear:none`, `neckwear:tie`, plus pack parts |
| `earrings`, `back`, `held` | `<category>:none`, plus pack parts |

The bundled `packs/parts` pack adds 22 original wearables: headphones, beanie, top hat, chef hat,
party hat, crown, wide-brim hat, headband, hard hat, sunglasses, round glasses, eye patch, hoop and
stud earrings, scarf, bow tie, necklace, cape, microphone, coffee cup, book and magic wand.

A character wears one part per category; picking another part in that category replaces it.

## Recipes (scripts and agents)

```json
{
  "style": "cartoon",
  "parts": ["hair:curly", "eyes:sleepy", "facialHair:moustache", {"part": "chef-hat", "colors": ["#ffffff"]}],
  "colors": {"skin": "#8d5a3b", "hair": "#1b1311", "top": "#f1f1ef", "bottom": "#2f3a56", "shoes": "#2a221e", "accent": "#e84a5f"}
}
```

Parts you leave out keep the default figure's choice. MCP tools: `list_part_categories`,
`search_parts`, `create_character`, `update_character`, `randomize_character` (seeded, with
`keep` locks), `preview_character` and `import_parts_pack`. See [AGENTS.md](AGENTS.md).

## Custom part packs

A part pack is an asset pack with a `parts` list. Load it from **Wearables → Load a part pack…**
in the creator (or **Props → Load asset pack…**), or with the `import_parts_pack` MCP tool.
When a character wears a part from your pack, the part's definition is copied into the project
file, so the project renders anywhere, including desktop export and the CLI, without the pack.

```json
{
  "id": "my-parts",
  "name": "My parts",
  "license": "CC0-1.0",
  "parts": [{
    "id": "viking-helmet",
    "name": "Viking helmet",
    "category": "headwear",
    "anchor": "head",
    "layer": "front",
    "colors": ["#9aa0a6", "#f1f1ef"],
    "tags": ["viking", "warrior", "horns"],
    "styles": [],
    "hidesHair": true,
    "shapes": [
      {"t": "path", "d": "M -52 -6 C -56 -78 56 -78 52 -6 Z", "fill": "$0"},
      {"t": "path", "d": "M -48 -40 C -80 -50 -86 -86 -72 -104 C -70 -80 -58 -64 -40 -58 Z", "fill": "$1"},
      {"t": "path", "d": "M 48 -40 C 80 -50 86 -86 72 -104 C 70 -80 58 -64 40 -58 Z", "fill": "$1"},
      {"t": "rect", "x": -55, "y": -18, "w": 110, "h": 14, "r": 6, "fill": "$0d"}
    ]
  }]
}
```

| Field | Meaning |
|---|---|
| `category` | `headwear`, `eyewear`, `neckwear`, `earrings`, `back`, `held` (or a new one of your own) |
| `anchor` | where it attaches: `head`, `neck`, `torso`, `handR`, `handL` (see below) |
| `layer` | `front` (default) or `back` (behind the body: capes, backpacks) |
| `colors` | default colours for the `$0`, `$1`, ... slots; users can change them in the creator |
| `tags` | words for search and for agents ("winter", "chef", "podcast") |
| `styles` | `["cartoon"]`, `["stick"]`, or empty for both |
| `hidesHair` | hide the hair on top (tight hats) |
| `shapes` | the same shapes as props (see [packs/README.md](../packs/README.md)); `$0d` is slot 0 darker, `$1l` slot 1 lighter |

Anchor coordinates, y grows downward:

| Anchor | Origin | Size guide |
|---|---|---|
| `head` | centre of the head | head radius 50; eyes at y -2, mouth at y 26, ears at x ±48, top of the head at y -50 |
| `neck` | base of the neck, between the shoulders | torso half-width 44 (stretched for slim and broad bodies) |
| `torso` | middle of the chest | torso from y -40 to y +64 |
| `handR`, `handL` | centre of the screen-right / screen-left hand | hand radius 11 |

Parts are drawn at cartoon size; on stick figures the head parts shrink to fit the smaller
head (×0.8) and neck and torso parts to the line body (×0.62).

## What's next

The design doc's full plan replaces today's drawn-in-code figures with SVG part sets on a shared
skeleton, and adds a large library of parts made by the team (clothes per body type, eye and
mouth sets with lip-sync shapes). Recipes and part ids stay the same, so characters made now keep working.
