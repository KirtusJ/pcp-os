const CHARCOAL: u32 = 0x1c1c1c;
const GRAY: u32 = 0x7f7e7e;
const SILVER: u32 = 0xb9b5b5;
const WARM_WHITE: u32 = 0xdad6cb;
const COOL_WHITE: u32 = 0xdce2e0;

pub struct Palette {
    pub sheet: u32,
    pub text: u32,
    pub band: u32,
    pub title: u32,
    pub hint: u32,
    pub rule: u32,
    pub field_edge: u32,
    pub selection: u32,
    pub selected_text: u32,
}

#[derive(Clone, Copy, Default)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

impl Theme {
    pub fn other(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }

    pub fn switch_hint(self) -> &'static str {
        match self {
            Self::Light => "F6 Dark theme",
            Self::Dark => "F6 Light theme",
        }
    }

    pub fn palette(self) -> &'static Palette {
        match self {
            Self::Light => &Palette {
                sheet: WARM_WHITE,
                text: CHARCOAL,
                band: COOL_WHITE,
                title: CHARCOAL,
                hint: CHARCOAL,
                rule: SILVER,
                field_edge: GRAY,
                selection: CHARCOAL,
                selected_text: WARM_WHITE,
            },
            Self::Dark => &Palette {
                sheet: CHARCOAL,
                text: WARM_WHITE,
                band: CHARCOAL,
                title: COOL_WHITE,
                hint: SILVER,
                rule: GRAY,
                field_edge: GRAY,
                selection: SILVER,
                selected_text: CHARCOAL,
            },
        }
    }
}
