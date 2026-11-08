//! The four layouts. Same strict rules as the HTML builder: one text column, the same section
//! order and standard section names, real text only (no tables, text boxes, icons or skill bars),
//! contact details in the body, standard fonts, body text 10 pt or larger, at most two pages.
//! Layouts only change typography, colour and spacing, so the machine-read text is identical.
use crate::model::LayoutId;
use wordcraft_doc::Rgb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingStyle {
    /// A thin rule under the heading.
    Rule,
    /// A short bar to the left of the heading.
    Bar,
    /// No line; letter-spaced capitals only.
    Plain,
}

#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub id: LayoutId,
    pub name: &'static str,
    pub description: &'static str,
    pub body_font: &'static str,
    pub head_font: &'static str,
    /// Body size in points.
    pub size: f32,
    pub name_size: f32,
    pub name_bold: bool,
    pub accent: Rgb,
    pub ink: Rgb,
    pub muted: Rgb,
    pub rule: Rgb,
    pub upper_headings: bool,
    pub heading_style: HeadingStyle,
    /// Spacing multiplier.
    pub gap: f32,
}

pub const LAYOUTS: [Layout; 4] = [
    Layout {
        id: LayoutId::Classic,
        name: "Klassisk",
        description: "Rolig og tidløs. Passer de fleste bransjer.",
        body_font: "Arial",
        head_font: "Arial",
        size: 10.5,
        name_size: 24.0,
        name_bold: true,
        accent: Rgb(0x2F, 0x4A, 0x66),
        ink: Rgb(0x1F, 0x2C, 0x3A),
        muted: Rgb(0x5B, 0x6B, 0x7D),
        rule: Rgb(0xBE, 0xCA, 0xD7),
        upper_headings: true,
        heading_style: HeadingStyle::Rule,
        gap: 1.0,
    },
    Layout {
        id: LayoutId::Modern,
        name: "Moderne",
        description: "Moderne skrift og tydelig fargeaksent. Teknologi og konsulent.",
        body_font: "Calibri",
        head_font: "Calibri",
        size: 10.5,
        name_size: 25.0,
        name_bold: true,
        accent: Rgb(0x1D, 0x66, 0x70),
        ink: Rgb(0x18, 0x26, 0x2B),
        muted: Rgb(0x56, 0x6A, 0x70),
        rule: Rgb(0x1D, 0x66, 0x70),
        upper_headings: true,
        heading_style: HeadingStyle::Bar,
        gap: 1.02,
    },
    Layout {
        id: LayoutId::Nordic,
        name: "Nordisk",
        description: "Serif i navn og overskrifter, dempede farger. Rådgivning, offentlig sektor, ledelse.",
        body_font: "Arial",
        head_font: "Georgia",
        size: 10.5,
        name_size: 26.0,
        name_bold: false,
        accent: Rgb(0x3E, 0x5C, 0x55),
        ink: Rgb(0x1D, 0x28, 0x26),
        muted: Rgb(0x5F, 0x6F, 0x6B),
        rule: Rgb(0xC9, 0xD3, 0xCF),
        upper_headings: false,
        heading_style: HeadingStyle::Rule,
        gap: 1.04,
    },
    Layout {
        id: LayoutId::Compact,
        name: "Kompakt",
        description: "Tettere og nøktern. Når lang erfaring må få plass på to sider.",
        body_font: "Arial",
        head_font: "Arial",
        size: 10.0,
        name_size: 21.0,
        name_bold: true,
        accent: Rgb(0x22, 0x22, 0x22),
        ink: Rgb(0x1A, 0x1A, 0x1A),
        muted: Rgb(0x55, 0x55, 0x55),
        rule: Rgb(0x9A, 0x9A, 0x9A),
        upper_headings: true,
        heading_style: HeadingStyle::Plain,
        gap: 0.82,
    },
];

pub fn layout(id: LayoutId) -> &'static Layout {
    LAYOUTS.iter().find(|l| l.id == id).unwrap_or(&LAYOUTS[0])
}

impl Layout {
    pub fn heading(&self, text: &str) -> String {
        if self.upper_headings { text.to_uppercase() } else { text.to_string() }
    }
}
