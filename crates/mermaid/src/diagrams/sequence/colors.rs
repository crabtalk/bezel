//! Pele's `diagrams/sequence/colors.ts`.
//!
//! Mermaid asks the browser whether the first word of a box title is a color.
//! Pele has no browser, so it knows the color keywords and the rgb() and hsl()
//! notations. Other functions, such as calc() and var(), are not recognized.

use d3::string;
use regex::Regex;
use std::{collections::HashSet, sync::LazyLock};

const KEYWORDS: [&str; 196] = [
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkgrey",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "green",
    "greenyellow",
    "grey",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightgrey",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "rebeccapurple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
    "transparent",
    "currentcolor",
    "inherit",
    "initial",
    "unset",
    "revert",
    "accentcolor",
    "accentcolortext",
    "activetext",
    "buttonborder",
    "buttonface",
    "buttontext",
    "canvas",
    "canvastext",
    "field",
    "fieldtext",
    "graytext",
    "highlight",
    "highlighttext",
    "linktext",
    "mark",
    "marktext",
    "selecteditem",
    "selecteditemtext",
    "visitedtext",
    "activeborder",
    "activecaption",
    "appworkspace",
    "background",
    "buttonhighlight",
    "buttonshadow",
    "captiontext",
    "inactiveborder",
    "inactivecaption",
    "inactivecaptiontext",
    "infobackground",
    "infotext",
    "menu",
    "menutext",
    "scrollbar",
    "threeddarkshadow",
    "threedface",
    "threedhighlight",
    "threedlightshadow",
    "threedshadow",
    "window",
    "windowframe",
    "windowtext",
];

static KEYWORD_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| KEYWORDS.into_iter().collect());
static RE_TERM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i-u)^[+-]?(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)(?:e[+-]?[0-9]+)?(%|deg|grad|rad|turn)?$",
    )
    .expect("pattern")
});
static RE_SPACES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("[{}]+", string::SPACE)).expect("pattern"));

#[derive(Clone, Copy, PartialEq)]
enum K {
    Bad,
    Number,
    Percent,
    Angle,
    None,
}

fn kind(term: &str) -> K {
    if term.to_lowercase() == "none" {
        return K::None;
    }
    let Some(m) = RE_TERM.captures(term) else {
        return K::Bad;
    };
    match m.get(1).map(|unit| unit.as_str()) {
        None => K::Number,
        Some("%") => K::Percent,
        Some(_) => K::Angle,
    }
}

fn is_alpha(term: &str, modern: bool) -> bool {
    let k = kind(term);
    k == K::Number || k == K::Percent || (modern && k == K::None)
}

fn is_function(color: &str) -> bool {
    let hsl = color.starts_with("hsl");
    let open = color.find('(').unwrap_or(0);
    let name = &color[..open];
    if !matches!(name, "rgb" | "rgba" | "hsl" | "hsla") {
        return false;
    }
    if string::len(color) > 120 || color.find(')') != Some(color.len() - 1) {
        return false;
    }
    let inner = string::trim(&color[open + 1..color.len() - 1]);
    let mut parts: Vec<&str>;
    let mut alpha: Option<&str> = None;
    let legacy = inner.contains(',');
    if legacy {
        parts = inner.split(',').map(string::trim).collect();
        if parts.len() == 4 {
            alpha = parts.pop();
        }
    } else {
        let slash: Vec<&str> = inner.split('/').collect();
        if slash.len() > 2 {
            return false;
        }
        parts = RE_SPACES.split(string::trim(slash[0])).collect();
        if slash.len() == 2 {
            alpha = Some(string::trim(slash[1]));
        }
    }
    if parts.len() != 3 || alpha.is_some_and(|alpha| !is_alpha(alpha, !legacy)) {
        return false;
    }
    let kinds: Vec<K> = parts.iter().map(|part| kind(part)).collect();
    if legacy {
        if hsl {
            return (kinds[0] == K::Number || kinds[0] == K::Angle)
                && kinds[1] == K::Percent
                && kinds[2] == K::Percent;
        }
        return (kinds[0] == K::Number || kinds[0] == K::Percent)
            && kinds[1] == kinds[0]
            && kinds[2] == kinds[0];
    }
    for (i, &k) in kinds.iter().enumerate().take(3) {
        let ok = k == K::Number
            || k == K::None
            || if hsl && i == 0 {
                k == K::Angle
            } else {
                k == K::Percent
            };
        if !ok {
            return false;
        }
    }
    true
}

pub fn is_css_color(color: &str) -> bool {
    if color.contains('(') {
        return is_function(color);
    }
    KEYWORD_SET.contains(color.to_lowercase().as_str())
}
