//! Character parts: the creator's library.
//!
//! Every character is a recipe of parts plus colours. Two kinds of part share
//! one catalogue, so the creator, scripts and AI agents treat them alike:
//!
//! - **Built-in parts** are the drawn rig's own options, with ids like
//!   `hair:curly`, `eyes:lashes`, `body:broad` or `headwear:cap`.
//! - **Pack parts** (wearables) come from asset packs (`parts[]`, see
//!   `pack::PartDef`): shapes attached to an anchor (head, neck, torso,
//!   hands). The bundled `packs/parts` pack and user packs work the same way.
//!
//! A recipe is plain data (`Recipe`), so an agent can turn "a grumpy chef with
//! a moustache" into `{"parts":["facialHair:moustache","chef-hat",...]}`.

use crate::geom::Color;
use crate::model::{Accessory, BrowStyle, Build, EyeStyle, FacialHair, FigureStyle, HairStyle, Look, NoseStyle, Project, Wear};
use crate::pack::{Library, Part, PartDef};
use crate::rig::Worn;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Creator categories in display order: (id, tab, label).
pub const CATEGORIES: &[(&str, &str, &str)] = &[
    ("body", "Body", "Body shape"),
    ("hair", "Hair", "Hair style"),
    ("eyes", "Face", "Eyes"),
    ("brows", "Face", "Eyebrows"),
    ("nose", "Face", "Nose"),
    ("facialHair", "Face", "Facial hair"),
    ("outfit", "Clothes", "Outfit"),
    ("headwear", "Wearables", "Headwear"),
    ("eyewear", "Wearables", "Eyewear"),
    ("neckwear", "Wearables", "Neckwear"),
    ("earrings", "Wearables", "Earrings"),
    ("back", "Wearables", "On the back"),
    ("held", "Wearables", "Held items"),
];

/// Categories that hold wearables (several categories, one part each).
pub const WEAR_CATEGORIES: &[&str] = &["headwear", "eyewear", "neckwear", "earrings", "back", "held"];

/// Colour fields on a look: (id, label).
pub const COLOR_FIELDS: &[(&str, &str)] =
    &[("skin", "Skin"), ("hair", "Hair"), ("top", "Top"), ("bottom", "Bottom"), ("shoes", "Shoes"), ("accent", "Accent")];

/// One entry of the parts catalogue (built-in or from a pack).
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PartInfo {
    pub id: String,
    pub name: String,
    pub category: String,
    /// "builtin" for the rig's own options, otherwise the pack id.
    pub pack: String,
    pub tags: Vec<String>,
    /// Figure styles it fits ("cartoon", "stick"); empty = both.
    pub styles: Vec<String>,
    /// Default colours of its own slots (pack parts only).
    pub colors: Vec<Color>,
    /// The look colour it uses, if any ("hair", "accent", ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_field: Option<&'static str>,
}

fn builtin(category: &str, value: &str, name: &str, tags: &str, styles: &[&str], color_field: Option<&'static str>) -> PartInfo {
    PartInfo {
        id: format!("{category}:{value}"),
        name: name.into(),
        category: category.into(),
        pack: "builtin".into(),
        tags: tags.split_whitespace().map(str::to_string).collect(),
        styles: styles.iter().map(|s| s.to_string()).collect(),
        colors: vec![],
        color_field,
    }
}

const BOTH: &[&str] = &[];
const CARTOON: &[&str] = &["cartoon"];
const STICK: &[&str] = &["stick"];

/// The rig's own options as parts.
pub fn builtin_parts() -> Vec<PartInfo> {
    let h = Some("hair");
    let a = Some("accent");
    vec![
        builtin("body", "slim", "Slim", "thin small lean kid teen", BOTH, None),
        builtin("body", "regular", "Regular", "average medium", BOTH, None),
        builtin("body", "broad", "Broad", "big strong large wide stocky", BOTH, None),
        builtin("hair", "bald", "Bald", "none shaved old", BOTH, None),
        builtin("hair", "short", "Short", "neat classic office", BOTH, h),
        builtin("hair", "spiky", "Spiky", "punk messy young energetic", BOTH, h),
        builtin("hair", "long", "Long", "straight flowing", BOTH, h),
        builtin("hair", "bun", "Bun", "updo tidy chef teacher", BOTH, h),
        builtin("hair", "curly", "Curly", "afro fluffy volume", BOTH, h),
        builtin("eyes", "round", "Round", "default friendly", CARTOON, None),
        builtin("eyes", "dots", "Dots", "simple cute small minimal", CARTOON, None),
        builtin("eyes", "wide", "Wide", "big curious young excited", CARTOON, None),
        builtin("eyes", "sleepy", "Sleepy", "tired bored grumpy lazy heavy", CARTOON, None),
        builtin("eyes", "lashes", "Lashes", "eyelashes feminine pretty", CARTOON, None),
        builtin("eyes", "narrow", "Narrow", "squint suspicious serious stern", CARTOON, None),
        builtin("brows", "normal", "Normal", "default", CARTOON, h),
        builtin("brows", "thin", "Thin", "light delicate", CARTOON, h),
        builtin("brows", "thick", "Thick", "bushy heavy grumpy old stern", CARTOON, h),
        builtin("brows", "arched", "Arched", "elegant raised curious", CARTOON, h),
        builtin("brows", "none", "None", "no eyebrows minimal", CARTOON, None),
        builtin("nose", "button", "Button", "default small", CARTOON, None),
        builtin("nose", "dot", "Dot", "tiny minimal cute", CARTOON, None),
        builtin("nose", "round", "Round", "big clown bulbous chef", CARTOON, None),
        builtin("nose", "long", "Long", "pointy profile sharp", CARTOON, None),
        builtin("nose", "none", "None", "no nose minimal", CARTOON, None),
        builtin("facialHair", "none", "None", "clean shaven", BOTH, None),
        builtin("facialHair", "stubble", "Stubble", "unshaven scruffy shadow", BOTH, h),
        builtin("facialHair", "moustache", "Moustache", "mustache chef detective old", BOTH, h),
        builtin("facialHair", "goatee", "Goatee", "beard chin artist", BOTH, h),
        builtin("facialHair", "beard", "Beard", "bearded hipster lumberjack", BOTH, h),
        builtin("facialHair", "fullBeard", "Full beard", "bearded long wizard old grandpa", BOTH, h),
        builtin("outfit", "clothes", "Shirt and trousers", "default casual", BOTH, None),
        builtin("outfit", "dress", "Dress", "skirt formal party", STICK, a),
        builtin("headwear", "none", "None", "no hat", BOTH, None),
        builtin("headwear", "cap", "Cap", "baseball sporty casual", BOTH, a),
        builtin("headwear", "bow", "Hair bow", "ribbon cute", BOTH, a),
        builtin("eyewear", "none", "None", "no glasses", BOTH, None),
        builtin("eyewear", "glasses", "Glasses", "smart nerd reading teacher", BOTH, None),
        builtin("neckwear", "none", "None", "nothing", BOTH, None),
        builtin("neckwear", "tie", "Tie", "office business formal", BOTH, a),
        builtin("earrings", "none", "None", "nothing", BOTH, None),
        builtin("back", "none", "None", "nothing", BOTH, None),
        builtin("held", "none", "None", "empty hands", BOTH, None),
    ]
}

fn pack_info(lib_pack: &str, def: &PartDef) -> PartInfo {
    PartInfo {
        id: def.id.clone(),
        name: def.name.clone(),
        category: def.category.clone(),
        pack: lib_pack.to_string(),
        tags: def.tags.clone(),
        styles: def.styles.clone(),
        colors: def.colors.clone(),
        color_field: None,
    }
}

/// Every part: built-in first, then pack parts by category. `style` keeps
/// only the parts that fit that figure style.
pub fn catalog(lib: &Library, style: Option<&str>) -> Vec<PartInfo> {
    let mut out = builtin_parts();
    let order = |c: &str| CATEGORIES.iter().position(|e| e.0 == c).unwrap_or(usize::MAX);
    let mut packs: Vec<PartInfo> = lib.parts.values().map(|p| pack_info(&lib.part_pack(&p.def.id), &p.def)).collect();
    packs.sort_by(|a, b| order(&a.category).cmp(&order(&b.category)).then(a.name.cmp(&b.name)));
    out.extend(packs);
    if let Some(s) = style {
        out.retain(|p| p.styles.is_empty() || p.styles.iter().any(|x| x == s));
    }
    out
}

/// Find parts by words (name, tags, category), best matches first.
pub fn search(lib: &Library, query: &str, category: Option<&str>, style: Option<&str>) -> Vec<PartInfo> {
    let words: Vec<String> = query.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
    let mut scored: Vec<(usize, PartInfo)> = catalog(lib, style)
        .into_iter()
        .filter(|p| category.is_none_or(|c| p.category == c))
        .filter_map(|p| {
            if words.is_empty() {
                return Some((0, p));
            }
            let hay = format!("{} {} {} {}", p.name, p.category, p.id, p.tags.join(" ")).to_lowercase();
            let score = words.iter().filter(|w| hay.contains(w.as_str())).count();
            (score > 0).then_some((score, p))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, p)| p).collect()
}

fn style_name(look: &Look) -> &'static str {
    match look.style {
        FigureStyle::Cartoon => "cartoon",
        FigureStyle::Stick => "stick",
    }
}

fn enum_value<T: serde::de::DeserializeOwned>(v: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(v.to_string())).ok()
}

fn legacy_category(a: Accessory) -> Option<&'static str> {
    match a {
        Accessory::Glasses => Some("eyewear"),
        Accessory::Cap | Accessory::Bow => Some("headwear"),
        Accessory::Tie => Some("neckwear"),
        Accessory::None => None,
    }
}

/// Look up a pack part's definition: the project's copy first, then the library.
pub fn part_def<'a>(project: Option<&'a Project>, lib: &'a Library, id: &str) -> Option<&'a PartDef> {
    project.and_then(|p| p.parts.get(id)).or_else(|| lib.parts.get(id).map(|p| &p.def))
}

/// Put one part on a look. `colors` overrides the part's colours (for a
/// built-in part, the first colour sets its look colour, e.g. the hair).
pub fn apply(look: &mut Look, lib: &Library, project: Option<&Project>, id: &str, colors: &[Color]) -> Result<(), String> {
    let first = colors.first().copied();
    if let Some((cat, value)) = id.split_once(':') {
        let bad = || format!("unknown part {id:?}; search_parts lists the valid ids");
        match cat {
            "body" => look.build = enum_value::<Build>(value).ok_or_else(bad)?,
            "hair" => {
                look.hair_style = enum_value::<HairStyle>(value).ok_or_else(bad)?;
                if let Some(c) = first {
                    look.hair = c;
                }
            }
            "eyes" => look.eyes = enum_value::<EyeStyle>(value).ok_or_else(bad)?,
            "brows" => {
                look.brows = enum_value::<BrowStyle>(value).ok_or_else(bad)?;
                if let Some(c) = first {
                    look.hair = c;
                }
            }
            "nose" => look.nose = enum_value::<NoseStyle>(value).ok_or_else(bad)?,
            "facialHair" => {
                look.facial_hair = enum_value::<FacialHair>(value).ok_or_else(bad)?;
                if let Some(c) = first {
                    look.hair = c;
                }
            }
            "outfit" => {
                look.dress = match value {
                    "dress" => true,
                    "clothes" => false,
                    _ => return Err(bad()),
                };
                if let (true, Some(c)) = (look.dress, first) {
                    look.accent = c;
                }
            }
            c if WEAR_CATEGORIES.contains(&c) => {
                clear_category(look, lib, project, c);
                if value != "none" {
                    let acc = enum_value::<Accessory>(value).filter(|a| legacy_category(*a) == Some(c)).ok_or_else(bad)?;
                    look.accessory = acc;
                    if let Some(col) = first {
                        look.accent = col;
                    }
                }
            }
            _ => return Err(bad()),
        }
        return Ok(());
    }
    let def = part_def(project, lib, id).ok_or_else(|| format!("unknown part {id:?}; load its part pack first"))?;
    let cat = def.category.clone();
    clear_category(look, lib, project, &cat);
    look.wear.push(Wear { part: id.to_string(), colors: colors.to_vec() });
    Ok(())
}

/// Remove whatever the look wears in a wearable category.
pub fn clear_category(look: &mut Look, lib: &Library, project: Option<&Project>, category: &str) {
    if legacy_category(look.accessory) == Some(category) {
        look.accessory = Accessory::None;
    }
    look.wear.retain(|w| part_def(project, lib, &w.part).is_none_or(|d| d.category != category));
}

/// Which part the look uses in each category (wearable categories may be "…:none").
pub fn selected(look: &Look, lib: &Library, project: Option<&Project>) -> BTreeMap<String, String> {
    let v = |x: serde_json::Value| x.as_str().unwrap_or_default().to_string();
    let mut m = BTreeMap::new();
    m.insert("body".into(), format!("body:{}", v(serde_json::to_value(look.build).unwrap())));
    m.insert("hair".into(), format!("hair:{}", v(serde_json::to_value(look.hair_style).unwrap())));
    m.insert("eyes".into(), format!("eyes:{}", v(serde_json::to_value(look.eyes).unwrap())));
    m.insert("brows".into(), format!("brows:{}", v(serde_json::to_value(look.brows).unwrap())));
    m.insert("nose".into(), format!("nose:{}", v(serde_json::to_value(look.nose).unwrap())));
    m.insert("facialHair".into(), format!("facialHair:{}", v(serde_json::to_value(look.facial_hair).unwrap())));
    m.insert("outfit".into(), if look.dress { "outfit:dress".into() } else { "outfit:clothes".into() });
    for c in WEAR_CATEGORIES {
        m.insert(c.to_string(), format!("{c}:none"));
    }
    if let Some(c) = legacy_category(look.accessory) {
        m.insert(c.into(), format!("{c}:{}", v(serde_json::to_value(look.accessory).unwrap())));
    }
    for w in &look.wear {
        if let Some(d) = part_def(project, lib, &w.part) {
            m.insert(d.category.clone(), w.part.clone());
        }
    }
    m
}

/// The pack parts a character wears, compiled and coloured for drawing.
pub fn worn(project: &Project, lib: &Library, look: &Look) -> Vec<Worn> {
    look.wear
        .iter()
        .filter_map(|w| {
            let owned;
            let part: &Part = match lib.parts.get(&w.part) {
                Some(p) => p,
                None => {
                    owned = Part::compile(project.parts.get(&w.part)?).ok()?;
                    &owned
                }
            };
            Some(Worn { anchor: part.def.anchor, layer: part.def.layer, hides_hair: part.def.hides_hair, face: part.def.category == "eyewear", ops: part.drawing.ops(&w.colors) })
        })
        .collect()
}

/// Copy the definitions of every non-built-in part a project's characters wear
/// into `project.parts`, so the file renders anywhere. Returns the ids added.
pub fn embed_used_parts(project: &mut Project, lib: &Library) -> Vec<String> {
    let builtin = &Library::builtin().parts;
    let mut used = BTreeSet::new();
    for s in &project.scenes {
        for it in &s.items {
            if let crate::model::ItemKind::Character(c) = &it.kind {
                used.extend(c.look.wear.iter().map(|w| w.part.clone()));
            }
        }
    }
    let mut added = vec![];
    for id in used {
        if builtin.contains_key(&id) || project.parts.contains_key(&id) {
            continue;
        }
        if let Some(p) = lib.parts.get(&id) {
            project.parts.insert(id.clone(), p.def.clone());
            added.push(id);
        }
    }
    added
}

// ---------------------------------------------------------------------------
// Recipes
// ---------------------------------------------------------------------------

/// A part in a recipe: an id, or an id with colours.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RecipePart {
    Id(String),
    Colored { part: String, #[serde(default)] colors: Vec<Color> },
}

impl RecipePart {
    fn split(&self) -> (&str, &[Color]) {
        match self {
            RecipePart::Id(id) => (id, &[]),
            RecipePart::Colored { part, colors } => (part, colors),
        }
    }
}

/// A character as data: a figure style, parts and colours. Parts not listed
/// keep the base look's choice (the default cartoon or stick figure).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Recipe {
    /// "cartoon" (default) or "stick".
    pub style: Option<String>,
    pub parts: Vec<RecipePart>,
    /// Look colours by field: skin, hair, top, bottom, shoes, accent.
    pub colors: BTreeMap<String, Color>,
}

impl Recipe {
    /// Build a look from this recipe, on top of `base` (or a default figure).
    pub fn to_look(&self, base: Option<&Look>, lib: &Library, project: Option<&Project>) -> Result<Look, String> {
        let stick = match self.style.as_deref() {
            None => base.is_some_and(|b| b.style == FigureStyle::Stick),
            Some("stick") => true,
            Some("cartoon") => false,
            Some(s) => return Err(format!("unknown style {s:?}: use \"cartoon\" or \"stick\"")),
        };
        let mut look = match base {
            Some(b) if (b.style == FigureStyle::Stick) == stick => b.clone(),
            _ if stick => Look::stick(false),
            _ => Look::default(),
        };
        for p in &self.parts {
            let (id, colors) = p.split();
            apply(&mut look, lib, project, id, colors)?;
        }
        for (field, c) in &self.colors {
            set_color(&mut look, field, *c)?;
        }
        Ok(look)
    }

    /// Describe a look as a recipe (every category, plus all colours).
    pub fn from_look(look: &Look, lib: &Library, project: Option<&Project>) -> Recipe {
        let sel = selected(look, lib, project);
        // Only categories that apply to this figure style (no cartoon eyes on stick figures).
        let fits: BTreeSet<String> = catalog(lib, Some(style_name(look))).into_iter().map(|p| p.category).collect();
        let mut parts: Vec<RecipePart> = CATEGORIES
            .iter()
            .filter(|(c, _, _)| fits.contains(*c))
            .filter_map(|(c, _, _)| sel.get(*c))
            .filter(|id| !id.ends_with(":none") || id.starts_with("brows") || id.starts_with("nose") || id.starts_with("facialHair"))
            .filter(|id| id.contains(':'))
            .map(|id| RecipePart::Id(id.clone()))
            .collect();
        parts.extend(look.wear.iter().map(|w| if w.colors.is_empty() { RecipePart::Id(w.part.clone()) } else { RecipePart::Colored { part: w.part.clone(), colors: w.colors.clone() } }));
        let colors = COLOR_FIELDS.iter().map(|(f, _)| (f.to_string(), get_color(look, f).unwrap())).collect();
        Recipe { style: Some(style_name(look).into()), parts, colors }
    }
}

pub fn get_color(look: &Look, field: &str) -> Option<Color> {
    Some(match field {
        "skin" => look.skin,
        "hair" => look.hair,
        "top" => look.top,
        "bottom" => look.bottom,
        "shoes" => look.shoes,
        "accent" => look.accent,
        _ => return None,
    })
}

pub fn set_color(look: &mut Look, field: &str, c: Color) -> Result<(), String> {
    let slot = match field {
        "skin" => &mut look.skin,
        "hair" => &mut look.hair,
        "top" => &mut look.top,
        "bottom" => &mut look.bottom,
        "shoes" => &mut look.shoes,
        "accent" => &mut look.accent,
        _ => return Err(format!("unknown colour {field:?}: use skin, hair, top, bottom, shoes or accent")),
    };
    *slot = c;
    Ok(())
}

// ---------------------------------------------------------------------------
// Randomise
// ---------------------------------------------------------------------------

pub const SKIN_TONES: &[&str] = &["#ffdbc1", "#f3c9a5", "#e9b48a", "#d7a37a", "#c68b62", "#b07850", "#8d5a3b", "#6b4329", "#4a2f1d"];
pub const HAIR_COLORS: &[&str] = &["#1b1311", "#2d2d2d", "#3b2a20", "#5a3825", "#8b5a2b", "#d99a3e", "#e8c27a", "#b5452b", "#9aa0a6", "#e9e6df"];
pub const CLOTHES: &[&str] = &[
    "#f26b5b", "#e84a5f", "#ffb703", "#3fae6a", "#219ebc", "#4f7cff", "#9b5de5", "#2f3a56", "#454d72", "#264653", "#e76f51", "#1f2233",
    "#f1f1ef", "#7a8b99", "#6d4c41", "#2a9d8f",
];
pub const SHOES: &[&str] = &["#2a221e", "#1f2233", "#5b3a29", "#f1f1ef", "#8a8f99", "#b5452b"];

/// Small deterministic generator, so a seed gives the same character everywhere.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn chance(&mut self, p: f32) -> bool {
        (self.next() % 1000) as f32 / 1000.0 < p
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len())]
    }
}

/// A random character in the look's style. Categories in `locks` (or colour
/// fields: skin, hair, clothes) keep their current value.
pub fn randomize(look: &Look, lib: &Library, project: Option<&Project>, seed: u64, locks: &[String]) -> Look {
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    for _ in 0..4 {
        r.next();
    }
    let locked = |c: &str| locks.iter().any(|l| l == c);
    let style = style_name(look);
    let cat = catalog(lib, Some(style));
    let mut out = look.clone();
    let col = |s: &str| Color::parse(s).unwrap();
    for (c, _, _) in CATEGORIES {
        if locked(c) {
            continue;
        }
        let options: Vec<&PartInfo> = cat.iter().filter(|p| p.category == *c).collect();
        if options.is_empty() {
            continue;
        }
        let wearable = WEAR_CATEGORIES.contains(c);
        // Most people wear nothing in a given wearable category; most have no beard.
        let pick = if wearable && !r.chance(0.3) {
            format!("{c}:none")
        } else if *c == "facialHair" && !r.chance(0.35) {
            "facialHair:none".to_string()
        } else {
            let real: Vec<&&PartInfo> = options.iter().filter(|p| !p.id.ends_with(":none") || !wearable).collect();
            if real.is_empty() {
                continue;
            }
            r.pick(&real).id.clone()
        };
        // Pack parts get a random first colour when they have slots.
        let colors: Vec<Color> = match cat.iter().find(|p| p.id == pick) {
            Some(p) if !p.colors.is_empty() => {
                let mut cs = p.colors.clone();
                cs[0] = col(r.pick(CLOTHES));
                cs
            }
            _ => vec![],
        };
        let _ = apply(&mut out, lib, project, &pick, &colors);
    }
    if style == "cartoon" {
        if !locked("skin") {
            out.skin = col(r.pick(SKIN_TONES));
        }
        if !locked("clothes") {
            out.top = col(r.pick(CLOTHES));
            out.bottom = col(r.pick(CLOTHES));
            out.shoes = col(r.pick(SHOES));
        }
    }
    if !locked("hair") && !locked("hairColor") {
        out.hair = col(r.pick(HAIR_COLORS));
    }
    if !locked("clothes") {
        out.accent = col(r.pick(CLOTHES));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_part_applies_and_reads_back() {
        let lib = Library::builtin();
        for p in builtin_parts() {
            let mut look = if p.styles == ["stick"] { Look::stick(false) } else { Look::default() };
            apply(&mut look, lib, None, &p.id, &[]).unwrap_or_else(|e| panic!("{}: {e}", p.id));
            assert_eq!(selected(&look, lib, None)[&p.category], p.id, "{}", p.id);
        }
    }

    #[test]
    fn bundled_wearables_load_and_swap_within_a_category() {
        let lib = Library::builtin();
        let wear: Vec<_> = catalog(lib, None).into_iter().filter(|p| p.pack != "builtin").collect();
        assert!(wear.len() >= 12, "bundled parts pack has {} parts", wear.len());
        let hats: Vec<_> = wear.iter().filter(|p| p.category == "headwear").collect();
        let mut look = Look { accessory: Accessory::Cap, ..Look::default() };
        apply(&mut look, lib, None, &hats[0].id, &[]).unwrap();
        assert_eq!(look.accessory, Accessory::None, "a hat replaces the cap");
        apply(&mut look, lib, None, &hats[1].id, &[]).unwrap();
        assert_eq!(look.wear.len(), 1, "one part per category");
        apply(&mut look, lib, None, "headwear:none", &[]).unwrap();
        assert!(look.wear.is_empty());
    }

    #[test]
    fn recipes_round_trip_and_search_finds_tags() {
        let lib = Library::builtin();
        let r: Recipe = serde_json::from_str(
            r##"{"parts":["hair:curly","eyes:sleepy","facialHair:moustache","brows:thick",{"part":"chef-hat","colors":["#ffffff"]}],"colors":{"skin":"#8d5a3b","top":"#f1f1ef"}}"##,
        )
        .unwrap();
        let look = r.to_look(None, lib, None).unwrap();
        assert_eq!(look.hair_style, HairStyle::Curly);
        assert_eq!(look.facial_hair, FacialHair::Moustache);
        assert_eq!(look.wear[0].part, "chef-hat");
        let back = Recipe::from_look(&look, lib, None).to_look(None, lib, None).unwrap();
        assert_eq!(back, look);
        assert_eq!(search(lib, "grumpy chef", None, None).first().map(|p| p.category.as_str()).is_some(), true);
        assert!(search(lib, "chef", Some("headwear"), None).iter().any(|p| p.id == "chef-hat"));
        assert!(Recipe { style: Some("robot".into()), ..Default::default() }.to_look(None, lib, None).is_err());
    }

    #[test]
    fn randomize_is_seeded_and_respects_locks() {
        let lib = Library::builtin();
        let base = Look::default();
        let a = randomize(&base, lib, None, 7, &[]);
        assert_eq!(a, randomize(&base, lib, None, 7, &[]));
        assert_ne!(a, randomize(&base, lib, None, 8, &[]));
        let locked = randomize(&base, lib, None, 9, &["hair".into(), "skin".into()]);
        assert_eq!(locked.hair_style, base.hair_style);
        assert_eq!(locked.skin, base.skin);
        let stick = randomize(&Look::stick(true), lib, None, 3, &[]);
        assert_eq!(stick.style, FigureStyle::Stick);
        assert_eq!(stick.eyes, EyeStyle::Round, "stick figures get no cartoon-only parts");
    }
}
