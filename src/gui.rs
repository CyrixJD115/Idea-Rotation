//! The Iced GUI: dark, compact, three tabs (Generate / Namespaces / Presets).
//! Design language: quiet surfaces, one blue accent, ghost buttons, no chrome
//! that isn't content.

use crate::config::{Config, Namespace, OptionEntry, Preset};
use crate::engine::{self, Idea};
use crate::theme as ui;
use iced::font::{Font, Weight};
use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, text, text_input,
    Space,
};
use iced::{Alignment, Element, Length, Pixels, Size, Task};
use std::path::PathBuf;

const ICON: &[u8] = include_bytes!("../assets/icon.png");
const REGULAR: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/NotoSans-SemiBold.ttf");

const SANS: Font = Font::with_name("Noto Sans");
const MEDIUM: Font = Font { weight: Weight::Semibold, ..SANS };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Generate,
    Namespaces,
    Presets,
}

#[derive(Debug, Clone)]
pub enum Message {
    Generate,
    Save,
    Reload,
    SwitchTab(Tab),
    SeedChanged(String),
    PatternSelected(String),
    CountUp,
    CountDown,
    CopyIdea(usize),

    NsSelected(usize),
    NsNameChanged(String),
    NsDescChanged(String),
    NsToggle(usize),
    NsAdd,
    NsDelete(usize),

    OptWordDraft(String),
    OptDescDraft(String),
    OptAdd,
    OptWordChanged(usize, String),
    OptDescChanged(usize, String),
    OptDelete(usize),

    PresetNameDraft(String),
    PresetSave,
    PresetApply(usize),
    PresetDelete(usize),
}

pub struct App {
    path: PathBuf,
    config: Config,
    ideas: Vec<Idea>,
    seed: String,
    pattern_choice: String,
    tab: Tab,
    selected_ns: Option<usize>,
    new_opt_word: String,
    new_opt_desc: String,
    preset_name: String,
    status: Option<String>,
    dirty: bool,
}

pub fn run(path: PathBuf) -> iced::Result {
    iced::application("Idea Rotation", update, view)
        .theme(|_| ui::theme())
        .window(iced::window::Settings {
            size: Size::new(620.0, 620.0),
            min_size: Some(Size::new(540.0, 540.0)),
            resizable: true,
            icon: window_icon(),
            ..Default::default()
        })
        .default_font(SANS)
        .font(REGULAR)
        .font(SEMIBOLD)
        .settings(iced::Settings {
            default_text_size: Pixels(13.5),
            ..Default::default()
        })
        .run_with(|| (App::new(path), Task::none()))
}

/// Decode the embedded PNG into a window icon (best effort).
fn window_icon() -> Option<iced::window::Icon> {
    let decoder = png::Decoder::new(std::io::Cursor::new(ICON));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    iced::window::icon::from_rgba(buf, info.width, info.height).ok()
}

impl App {
    fn new(path: PathBuf) -> App {
        let (config, status) = match Config::load(&path) {
            Ok(c) => (c, None),
            Err(e) => (
                Config::default(),
                Some(format!("{e} — loaded built-in defaults; Save writes the file")),
            ),
        };
        App {
            selected_ns: (!config.namespaces.is_empty()).then_some(0),
            path,
            config,
            ideas: Vec::new(),
            seed: String::new(),
            pattern_choice: "auto".into(),
            tab: Tab::Generate,
            new_opt_word: String::new(),
            new_opt_desc: String::new(),
            preset_name: String::new(),
            status,
            dirty: false,
        }
    }

    fn fix_selection(&mut self) {
        if self.selected_ns.is_some_and(|i| i >= self.config.namespaces.len()) {
            self.selected_ns = if self.config.namespaces.is_empty() {
                None
            } else {
                Some(self.config.namespaces.len() - 1)
            };
        }
    }

    fn selected_ns_mut(&mut self) -> Option<&mut Namespace> {
        let idx = self.selected_ns?;
        self.config.namespaces.get_mut(idx)
    }
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

fn update(state: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Generate => {
            let seed = state.seed.trim();
            let pattern = state.pattern_choice.trim();
            state.ideas = match engine::generate(
                &state.config,
                state.config.settings.count,
                None,
                (!seed.is_empty()).then_some(seed),
                (!pattern.is_empty()).then_some(pattern),
            ) {
                Ok(ideas) => ideas,
                Err(e) => {
                    state.ideas.clear();
                    state.status = Some(e);
                    return Task::none();
                }
            };
            state.status = Some(if state.ideas.is_empty() {
                "Nothing generated — enable a namespace that has words.".into()
            } else {
                format!("{} name(s)", state.ideas.len())
            });
        }
        Message::Save => {
            state.config.sanitize();
            state.fix_selection();
            state.status = Some(match state.config.save(&state.path) {
                Ok(()) => {
                    state.dirty = false;
                    format!("saved — {}", state.path.display())
                }
                Err(e) => e,
            });
        }
        Message::Reload => {
            state.status = Some(match Config::load(&state.path) {
                Ok(cfg) => {
                    state.config = cfg;
                    state.ideas.clear();
                    state.dirty = false;
                    state.fix_selection();
                    format!("reloaded — {}", state.path.display())
                }
                Err(e) => e,
            });
        }
        Message::SwitchTab(tab) => state.tab = tab,
        Message::SeedChanged(s) => state.seed = s,
        Message::PatternSelected(p) => state.pattern_choice = p,
        Message::CountUp => {
            state.config.settings.count = (state.config.settings.count + 1).min(12);
            state.dirty = true;
        }
        Message::CountDown => {
            state.config.settings.count = state.config.settings.count.max(2) - 1;
            state.dirty = true;
        }
        Message::CopyIdea(i) => {
            if let Some(idea) = state.ideas.get(i) {
                let name = idea.name.clone();
                state.status = Some(format!("copied — {name}"));
                return iced::clipboard::write(name);
            }
        }

        // -- namespaces ----------------------------------------------------
        Message::NsSelected(i) => {
            if i < state.config.namespaces.len() {
                state.selected_ns = Some(i);
            }
        }
        Message::NsNameChanged(s) => {
            if let Some(ns) = state.selected_ns_mut() {
                ns.name = s;
                state.dirty = true;
            }
        }
        Message::NsDescChanged(s) => {
            if let Some(ns) = state.selected_ns_mut() {
                ns.description = s;
                state.dirty = true;
            }
        }
        Message::NsToggle(i) => {
            if let Some(ns) = state.config.namespaces.get_mut(i) {
                ns.enabled = !ns.enabled;
                state.dirty = true;
            }
        }
        Message::NsAdd => {
            let taken: Vec<String> =
                state.config.namespaces.iter().map(|n| n.id.clone()).collect();
            state.config.namespaces.push(Namespace::new("New namespace", &taken));
            state.selected_ns = Some(state.config.namespaces.len() - 1);
            state.dirty = true;
            state.status = Some("namespace added — give it words".into());
        }
        Message::NsDelete(i) => {
            if i < state.config.namespaces.len() {
                state.config.namespaces.remove(i);
                state.fix_selection();
                state.dirty = true;
            }
        }

        // -- options -------------------------------------------------------
        Message::OptWordDraft(s) => state.new_opt_word = s,
        Message::OptDescDraft(s) => state.new_opt_desc = s,
        Message::OptAdd => {
            let word = state.new_opt_word.trim().to_string();
            if word.is_empty() {
                state.status = Some("a word needs a name".into());
            } else {
                let description = state.new_opt_desc.trim().to_string();
                if let Some(ns) = state.selected_ns_mut() {
                    ns.options.push(OptionEntry { word, description });
                    state.dirty = true;
                }
                state.new_opt_word.clear();
                state.new_opt_desc.clear();
            }
        }
        Message::OptWordChanged(i, s) => {
            if let Some(opt) = state.selected_ns_mut().and_then(|ns| ns.options.get_mut(i)) {
                opt.word = s;
                state.dirty = true;
            }
        }
        Message::OptDescChanged(i, s) => {
            if let Some(opt) = state.selected_ns_mut().and_then(|ns| ns.options.get_mut(i)) {
                opt.description = s;
                state.dirty = true;
            }
        }
        Message::OptDelete(i) => {
            if let Some(ns) = state.selected_ns_mut() {
                if i < ns.options.len() {
                    ns.options.remove(i);
                    state.dirty = true;
                }
            }
        }

        // -- presets -------------------------------------------------------
        Message::PresetNameDraft(s) => state.preset_name = s,
        Message::PresetSave => {
            let name = state.preset_name.trim().to_string();
            if name.is_empty() {
                state.status = Some("a preset needs a name".into());
            } else {
                let enabled: Vec<String> = state
                    .config
                    .namespaces
                    .iter()
                    .filter(|n| n.enabled)
                    .map(|n| n.id.clone())
                    .collect();
                let preset = Preset { name: name.clone(), enabled };
                match state.config.presets.iter().position(|p| p.name == name) {
                    Some(i) => state.config.presets[i] = preset,
                    None => state.config.presets.push(preset),
                }
                state.preset_name.clear();
                state.dirty = true;
                state.status = Some(format!("preset “{name}” saved"));
            }
        }
        Message::PresetApply(i) => {
            if let Some(preset) = state.config.presets.get(i) {
                let enabled = preset.enabled.clone();
                let name = preset.name.clone();
                for ns in &mut state.config.namespaces {
                    ns.enabled = enabled.contains(&ns.id);
                }
                state.tab = Tab::Generate;
                state.dirty = true;
                state.status = Some(format!("preset “{name}” applied"));
            }
        }
        Message::PresetDelete(i) => {
            if i < state.config.presets.len() {
                state.config.presets.remove(i);
                state.dirty = true;
            }
        }
    }
    Task::none()
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

fn tab_button<'a>(label: &'a str, tab: Tab, active: bool) -> button::Button<'a, Message> {
    button(text(label).size(12.5).font(if active { MEDIUM } else { Font::DEFAULT }))
        .on_press(Message::SwitchTab(tab))
        .padding([5, 13])
        .style(move |_t, s| if active { ui::tab_active(_t, s) } else { ui::tab_idle(_t, s) })
}

fn field<'a>(placeholder: &str, value: &str, on_input: impl Fn(String) -> Message + 'a) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value)
        .on_input(on_input)
        .size(13)
        .padding([6, 9])
        .style(ui::field)
}

fn micro(label: impl Into<String>) -> iced::widget::Text<'static> {
    text(label.into()).size(10).color(ui::DIM)
}

fn view(state: &App) -> Element<'_, Message> {
    let header = row![
        column![
            text("Idea Rotation").font(MEDIUM).size(16),
            text("config-driven idea & name generator").size(10.5).color(ui::DIM),
        ]
        .spacing(1),
        Space::with_width(Length::Fill),
        button(text("reload").size(11.5))
            .on_press(Message::Reload)
            .padding([5, 9])
            .style(ui::ghost),
        button(
            row![
                text("save").size(11.5).font(if state.dirty { MEDIUM } else { Font::DEFAULT }),
                text(if state.dirty { " •" } else { "" }).size(11.5).color(ui::ACCENT_BRIGHT),
            ]
            .spacing(1)
        )
        .on_press(Message::Save)
        .padding([5, 11])
        .style(ui::save_button(state.dirty)),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let nav = row![
        tab_button("Generate", Tab::Generate, state.tab == Tab::Generate),
        tab_button("Namespaces", Tab::Namespaces, state.tab == Tab::Namespaces),
        tab_button("Presets", Tab::Presets, state.tab == Tab::Presets),
    ]
    .spacing(3);

    let content: Element<Message> = match state.tab {
        Tab::Generate => generate_tab(state),
        Tab::Namespaces => namespaces_tab(state),
        Tab::Presets => presets_tab(state),
    };

    let footer = row![
        text(
            state
                .status
                .clone()
                .unwrap_or_else(|| "edits live in memory — save writes config.toml".into()),
        )
        .size(10.5)
        .color(ui::DIM),
        Space::with_width(Length::Fill),
        text(state.path.display().to_string()).size(10).color(ui::DIM),
    ];

    column![header, nav, content, footer]
        .spacing(10)
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// -- Generate tab -----------------------------------------------------------

fn generate_tab(state: &App) -> Element<'_, Message> {
    let count = state.config.settings.count;

    let stepper = row![
        button(text("−").size(12))
            .on_press(Message::CountDown)
            .padding([4, 9])
            .style(ui::ghost),
        text(count.to_string()).size(13).font(MEDIUM).width(Length::Fixed(20.0)).align_x(iced::Center),
        button(text("+").size(12))
            .on_press(Message::CountUp)
            .padding([4, 9])
            .style(ui::ghost),
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    let mut pattern_options: Vec<String> = vec!["auto".into()];
    pattern_options.extend(state.config.patterns.iter().map(|p| p.name.clone()));

    let controls = row![
        stepper,
        field("seed — empty for random", &state.seed, Message::SeedChanged)
            .width(Length::Fill),
        pick_list(
            pattern_options,
            Some(state.pattern_choice.clone()),
            Message::PatternSelected,
        )
        .padding([6, 9])
        .width(Length::Fixed(150.0)),
        button(text("Generate").size(12.5).font(MEDIUM))
            .on_press(Message::Generate)
            .padding([7, 18])
            .style(button::primary),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let body: Element<Message> = if state.ideas.is_empty() {
        container(
            column![
                text("Roll your first idea")
                    .font(MEDIUM)
                    .size(15)
                    .color(ui::TEXT),
                text("words are picked from every enabled namespace and glued together — manage them under Namespaces")
                    .size(11.5)
                    .color(ui::DIM),
            ]
            .spacing(6)
            .align_x(iced::Center),
        )
        .width(Length::Fill)
        .padding([36, 12])
        .center(Length::Fill)
        .style(ui::card)
        .into()
    } else {
        let mut cards: Vec<Element<Message>> = vec![hero_card(0, &state.ideas[0])];
        for (i, idea) in state.ideas.iter().enumerate().skip(1) {
            cards.push(slim_idea(i, idea));
        }
        scrollable(column(cards).spacing(6)).height(Length::Fill).into()
    };

    column![controls, body].spacing(10).height(Length::Fill).into()
}

fn hero_card<'a>(index: usize, idea: &'a Idea) -> Element<'a, Message> {
    let headline = column![
        row![
            text(idea.name.clone()).font(MEDIUM).size(21),
            Space::with_width(Length::Fill),
            button(text("copy").size(11))
                .on_press(Message::CopyIdea(index))
                .padding([4, 10])
                .style(ui::ghost),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text(format!("via {}", idea.pattern.clone().unwrap_or_default()))
            .size(10.5)
            .color(ui::DIM),
    ]
    .spacing(3);
    let picks: Vec<Element<Message>> = idea
        .picks
        .iter()
        .map(|pick| {
            row![
                container(
                    text(pick.word.clone())
                        .size(12)
                        .font(MEDIUM)
                        .color(ui::ACCENT_BRIGHT)
                )
                .padding([2, 10])
                .style(ui::chip),
                text(pick.description.clone()).size(11.5).color(ui::DIM).width(Length::Fill),
            ]
            .spacing(7)
            .align_y(Alignment::Center)
            .into()
        })
        .collect();

    container(column![headline, column(picks).spacing(5)].spacing(9))
        .padding([16, 18])
        .style(ui::hero)
        .into()
}

fn slim_idea<'a>(index: usize, idea: &'a Idea) -> Element<'a, Message> {
    row![
        text(idea.name.clone())
            .size(13)
            .font(Font::DEFAULT)
            .color(ui::TEXT),
        Space::with_width(Length::Fill),
        button(text("copy").size(10.5))
            .on_press(Message::CopyIdea(index))
            .padding([3, 9])
            .style(ui::ghost),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .padding([9, 14])
    .into()
}

// -- Namespaces tab ---------------------------------------------------------

fn namespaces_tab(state: &App) -> Element<'_, Message> {
    let mut list: Vec<Element<Message>> = state
        .config
        .namespaces
        .iter()
        .enumerate()
        .map(|(i, ns)| {
            let selected = state.selected_ns == Some(i);
            row![
                checkbox("", ns.enabled)
                    .on_toggle(move |_| Message::NsToggle(i))
                    .size(14),
                button(
                    row![
                        text(ns.name.clone()).size(12.5).font(if selected {
                            MEDIUM
                        } else {
                            Font::DEFAULT
                        }),
                        text(format!("{}", ns.options.len())).size(10.5).color(ui::DIM),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .on_press(Message::NsSelected(i))
                .width(Length::Fill)
                .padding([5, 9])
                .style(move |_t, s| if selected { ui::nav_selected(_t, s) } else { ui::nav_idle(_t, s) }),
            ]
            .spacing(7)
            .align_y(Alignment::Center)
            .into()
        })
        .collect();

    list.push(
        button(row![text("＋").size(11), text("new namespace").size(11.5)].spacing(6))
            .on_press(Message::NsAdd)
            .width(Length::Fill)
            .padding([6, 9])
            .style(ui::ghost)
            .into(),
    );

    let sidebar = container(scrollable(column(list).spacing(2)).height(Length::Fill))
        .width(Length::Fixed(188.0))
        .padding(6)
        .style(ui::card);

    let editor: Element<Message> = match state.selected_ns {
        Some(i) => namespace_editor(state, i),
        None => container(
            text("select a namespace — or make a new one").size(11.5).color(ui::DIM),
        )
        .padding(24)
        .width(Length::Fill)
        .center(Length::Fill)
        .style(ui::card)
        .into(),
    };

    row![sidebar, editor]
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn namespace_editor(state: &App, i: usize) -> Element<'_, Message> {
    let Some(ns) = state.config.namespaces.get(i) else {
        return text("gone").into();
    };

    let title_row = row![
        text_input("namespace name", &ns.name)
            .on_input(Message::NsNameChanged)
            .size(14.5)
            .font(MEDIUM)
            .padding([7, 10])
            .style(ui::field)
            .width(Length::Fill),
        button(text("delete").size(11))
            .on_press(Message::NsDelete(i))
            .padding([5, 9])
            .style(ui::ghost_danger),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let desc_and_meta = column![
        field("what this slot adds to an idea", &ns.description, Message::NsDescChanged),
        row![
            micro(format!("id: {}", ns.id)),
            Space::with_width(Length::Fill),
            micro(if ns.enabled { "in the mix" } else { "paused" }),
        ],
    ]
    .spacing(5);

    let mut option_rows: Vec<Element<Message>> = Vec::new();
    for (oi, opt) in ns.options.iter().enumerate() {
        option_rows.push(
            row![
                text_input("word", &opt.word)
                    .on_input(move |s| Message::OptWordChanged(oi, s))
                    .size(12.5)
                    .font(MEDIUM)
                    .padding([5, 9])
                    .style(ui::field)
                    .width(Length::Fixed(140.0)),
                text_input("description", &opt.description)
                    .on_input(move |s| Message::OptDescChanged(oi, s))
                    .size(12.5)
                    .padding([5, 9])
                    .style(ui::field)
                    .width(Length::Fill),
                button(text("×").size(11))
                    .on_press(Message::OptDelete(oi))
                    .padding([4, 8])
                    .style(ui::ghost_danger),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .into(),
        );
    }
    if option_rows.is_empty() {
        option_rows.push(
            text("no words yet — add the first one below").size(11).color(ui::DIM).into(),
        );
    }

    let add_option = row![
        text_input("word", &state.new_opt_word)
            .on_input(Message::OptWordDraft)
            .size(12.5)
            .padding([5, 9])
            .style(ui::field)
            .width(Length::Fixed(140.0)),
        text_input("description", &state.new_opt_desc)
            .on_input(Message::OptDescDraft)
            .on_submit(Message::OptAdd)
            .size(12.5)
            .padding([5, 9])
            .style(ui::field)
            .width(Length::Fill),
        button(text("+ add").size(11.5))
            .on_press(Message::OptAdd)
            .padding([5, 11])
            .style(ui::soft),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    container(
        column![
            title_row,
            desc_and_meta,
            Space::with_height(Length::Fixed(2.0)),
            micro(format!("WORDS · {}", ns.options.len())),
            scrollable(column(option_rows).spacing(4))
                .height(Length::Fill)
                .spacing(3),
            add_option,
        ]
        .spacing(7)
        .height(Length::Fill),
    )
    .padding(12)
    .width(Length::Fill)
    .style(ui::card)
    .into()
}

// -- Presets tab ------------------------------------------------------------

fn presets_tab(state: &App) -> Element<'_, Message> {
    let save_row = row![
        field("name this mix — e.g. quick, heavy, brainstorm", &state.preset_name, Message::PresetNameDraft)
            .on_submit(Message::PresetSave)
            .width(Length::Fill),
        button(text("save mix").size(11.5).font(MEDIUM))
            .on_press(Message::PresetSave)
            .padding([6, 14])
            .style(ui::soft),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let caption = text("a preset remembers which namespaces are on right now — apply it later to jump back")
        .size(10.5)
        .color(ui::DIM);

    let mut preset_rows: Vec<Element<Message>> = state
        .config
        .presets
        .iter()
        .enumerate()
        .map(|(i, p)| {
            row![
                text(p.name.clone()).font(MEDIUM).size(13),
                text(format!("{} namespaces", p.enabled.len())).size(10.5).color(ui::DIM),
                Space::with_width(Length::Fill),
                button(text("apply").size(11))
                    .on_press(Message::PresetApply(i))
                    .padding([4, 11])
                    .style(ui::soft),
                button(text("×").size(11))
                    .on_press(Message::PresetDelete(i))
                    .padding([4, 8])
                    .style(ui::ghost_danger),
            ]
            .spacing(9)
            .align_y(Alignment::Center)
            .padding([8, 14])
            .into()
        })
        .collect();

    if preset_rows.is_empty() {
        preset_rows.push(
            text("no presets yet — switch some namespaces on, name it, save")
                .size(11.5)
                .color(ui::DIM)
                .into(),
        );
    }

    column![
        container(column![save_row, caption].spacing(6))
            .padding(12)
            .style(ui::card),
        scrollable(column(preset_rows).spacing(4)).height(Length::Fill),
    ]
    .spacing(8)
    .height(Length::Fill)
    .into()
}
