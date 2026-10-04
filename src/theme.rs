//! Dark theme with a blue accent — quiet surfaces, soft borders, one accent.

use iced::widget::{button, container, overlay, pick_list, text_input};
use iced::{theme, Background, Border, Color, Theme};

// -- palette ---------------------------------------------------------------

pub const BG: Color = Color::from_rgb(0.051, 0.063, 0.090); // #0D1017
pub const SURFACE: Color = Color::from_rgb(0.078, 0.094, 0.133); // #141822
pub const SURFACE_2: Color = Color::from_rgb(0.106, 0.129, 0.180); // #1B212E
pub const BORDER: Color = Color::from_rgb(0.145, 0.176, 0.235); // #252D3C
pub const ACCENT: Color = Color::from_rgb(0.239, 0.545, 0.992); // #3D8BFD
pub const ACCENT_BRIGHT: Color = Color::from_rgb(0.435, 0.694, 1.0); // #6FB1FF
pub const TEXT: Color = Color::from_rgb(0.914, 0.933, 0.965); // #E9EEF6
pub const DIM: Color = Color::from_rgb(0.596, 0.635, 0.702); // #98A2B3

fn accent_alpha(a: f32) -> Color {
    Color { a, ..ACCENT }
}

pub fn theme() -> Theme {
    Theme::custom(
        "Idea Rotation".into(),
        theme::Palette {
            background: BG,
            text: TEXT,
            primary: ACCENT,
            success: Color::from_rgb(0.180, 0.627, 0.263),
            danger: Color::from_rgb(0.973, 0.318, 0.286),
        },
    )
}

// -- containers --------------------------------------------------------------

/// Quiet card for grouped content.
pub fn card(_t: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        border: Border { width: 1.0, radius: 12.0.into(), color: BORDER },
        ..Default::default()
    }
}

/// The hero card holding the featured idea — accent-tinted edge.
pub fn hero(_t: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb(
            0.094, 0.125, 0.184, // #18203F-ish
        ))),
        border: Border { width: 1.0, radius: 14.0.into(), color: accent_alpha(0.38) },
        ..Default::default()
    }
}

/// Pill chip for a picked word.
pub fn chip(_t: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE_2)),
        border: Border {
            width: 1.0,
            radius: 999.0.into(),
            color: Color { a: 0.55, ..BORDER },
        },
        ..Default::default()
    }
}

// -- buttons -------------------------------------------------------------------

/// Active nav tab — soft accent pill.
pub fn tab_active(_t: &Theme, _s: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(accent_alpha(0.16))),
        text_color: ACCENT_BRIGHT,
        border: Border { width: 1.0, radius: 9.0.into(), color: accent_alpha(0.35) },
        ..Default::default()
    }
}

/// Inactive nav tab — just text.
pub fn tab_idle(_t: &Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered);
    button::Style {
        background: hovered.then(|| Background::Color(Color { a: 0.5, ..SURFACE_2 })),
        text_color: if hovered { TEXT } else { DIM },
        border: Border {
            width: 1.0,
            radius: 9.0.into(),
            color: if hovered { BORDER } else { Color::TRANSPARENT },
        },
        ..Default::default()
    }
}

/// Borderless text action (copy, apply, reload).
pub fn ghost(_t: &Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered);
    button::Style {
        background: hovered.then(|| Background::Color(Color { a: 0.6, ..SURFACE_2 })),
        text_color: if hovered { TEXT } else { DIM },
        border: Border { width: 0.0, radius: 8.0.into(), color: Color::TRANSPARENT },
        ..Default::default()
    }
}

/// Ghost but red — destructive actions.
pub fn ghost_danger(_t: &Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered);
    button::Style {
        background: hovered.then(|| Background::Color(Color { a: 0.12, ..Color::from_rgb(0.973, 0.318, 0.286) })),
        text_color: if hovered {
            Color::from_rgb(0.98, 0.45, 0.42)
        } else {
            DIM
        },
        border: Border { width: 0.0, radius: 8.0.into(), color: Color::TRANSPARENT },
        ..Default::default()
    }
}

/// Soft accent button — secondary actions (save-mix, add, apply-ish).
pub fn soft(_t: &Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered);
    button::Style {
        background: Some(Background::Color(if hovered {
            accent_alpha(0.24)
        } else {
            accent_alpha(0.15)
        })),
        text_color: ACCENT_BRIGHT,
        border: Border { width: 1.0, radius: 9.0.into(), color: accent_alpha(0.32) },
        ..Default::default()
    }
}

/// Save button: tinted when clean, solid accent when there are unsaved edits.
pub fn save_button(dirty: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_t: &Theme, status: button::Status| {
        let hovered = matches!(status, button::Status::Hovered);
        if dirty {
            button::Style {
                background: Some(Background::Color(if hovered {
                    ACCENT_BRIGHT
                } else {
                    ACCENT
                })),
                text_color: Color::from_rgb(0.06, 0.09, 0.14),
                border: Border { width: 0.0, radius: 9.0.into(), color: Color::TRANSPARENT },
                ..Default::default()
            }
        } else {
            button::Style {
                background: hovered.then(|| Background::Color(accent_alpha(0.22))),
                text_color: if hovered { TEXT } else { DIM },
                border: Border { width: 1.0, radius: 9.0.into(), color: BORDER },
                ..Default::default()
            }
        }
    }
}

/// Sidebar entry — selected gets an accent edge, others stay quiet.
pub fn nav_selected(_t: &Theme, _s: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(SURFACE_2)),
        text_color: TEXT,
        border: Border { width: 1.0, radius: 10.0.into(), color: accent_alpha(0.45) },
        ..Default::default()
    }
}

pub fn nav_idle(_t: &Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered);
    button::Style {
        background: hovered.then(|| Background::Color(Color { a: 0.55, ..SURFACE_2 })),
        text_color: if hovered { TEXT } else { DIM },
        border: Border {
            width: 1.0,
            radius: 10.0.into(),
            color: if hovered { BORDER } else { Color::TRANSPARENT },
        },
        ..Default::default()
    }
}

// -- inputs --------------------------------------------------------------------

/// Rounded field with an accent ring on focus.
pub fn field(_t: &Theme, status: text_input::Status) -> text_input::Style {
    let focused = matches!(status, text_input::Status::Focused);
    text_input::Style {
        background: Background::Color(SURFACE_2),
        border: Border {
            width: if focused { 1.4 } else { 1.0 },
            radius: 9.0.into(),
            color: if focused { ACCENT } else { BORDER },
        },
        icon: DIM,
        placeholder: Color::from_rgb(0.42, 0.46, 0.53),
        value: TEXT,
        selection: accent_alpha(0.35),
    }
}

// -- pick list (pattern dropdown) ----------------------------------------------

/// Closed dropdown: same surface as the input fields, accent ring on hover.
pub fn pick_list_style(_t: &Theme, status: pick_list::Status) -> pick_list::Style {
    let hovered = matches!(status, pick_list::Status::Hovered);
    pick_list::Style {
        text_color: if hovered { TEXT } else { DIM },
        placeholder_color: Color::from_rgb(0.42, 0.46, 0.53),
        handle_color: if hovered { ACCENT_BRIGHT } else { DIM },
        background: Background::Color(SURFACE_2),
        border: Border {
            width: 1.0,
            radius: 9.0.into(),
            color: if hovered { ACCENT } else { BORDER },
        },
    }
}

/// Open dropdown menu: elevated surface, accent-highlighted hovered option.
pub fn pick_list_menu(_t: &Theme) -> overlay::menu::Style {
    overlay::menu::Style {
        background: Background::Color(SURFACE),
        border: Border {
            width: 1.0,
            radius: 10.0.into(),
            color: BORDER,
        },
        text_color: DIM,
        selected_text_color: ACCENT_BRIGHT,
        selected_background: Background::Color(SURFACE_2),
    }
}
