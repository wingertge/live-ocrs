use iced::{
    theme,
    widget::{text, Row},
    Color, Element,
};

use crate::{
    app::OcrMessage,
    character::Block,
    dict::{Dictionary, DictionaryEntry, Pinyin, Tone},
};

pub struct Definitions {
    pub dict: Dictionary,
    pub ocr_strings: Vec<Block>,
    pub definitions: Vec<DictionaryEntry>,
}

impl Definitions {
    pub fn new(dict: Dictionary) -> Self {
        Self {
            dict,
            ocr_strings: Vec::new(),
            definitions: Vec::new(),
        }
    }

    pub fn view(&self) -> Vec<Element<OcrMessage>> {
        let definition_text = self.definitions.iter().flat_map(|definition| {
            let header = text(definition.simplified.to_owned()).size(20);
            let pinyin = view_pinyin(&definition.pinyin);
            let body = definition
                .translations
                .iter()
                .map(|translation| text(translation).size(16).into());
            vec![header.into(), pinyin].into_iter().chain(body)
        });
        vec![
            text("Definitions: ").size(24).into(),
            text("").size(16).into(),
        ]
        .into_iter()
        .chain(definition_text)
        .collect()
    }

    pub fn update(&mut self, text: &str) {
        self.definitions = self.dict.matches(text);
    }
}

fn pinyin_color(tone: Tone) -> Color {
    match tone {
        Tone::First => Color::new(227. / 255., 0.0, 0.0, 1.0),
        Tone::Second => Color::new(2. / 255., 179. / 255., 28. / 255., 1.0),
        Tone::Third => Color::new(21. / 255., 16. / 255., 240. / 255., 1.0),
        Tone::Fourth => Color::new(137. / 255., 0., 191. / 255., 1.0),
        Tone::Fifth => Color::new(119. / 255., 119. / 255., 119. / 255., 1.0),
        Tone::None => Color::WHITE,
    }
}

fn view_pinyin(pinyin: &[Pinyin]) -> Element<OcrMessage> {
    let elements = pinyin.iter().map(|it| {
        text(it.syllable.to_owned())
            .size(16)
            .style(theme::Text::Color(pinyin_color(it.tone)))
            .into()
    });
    Row::with_children(elements).into()
}
