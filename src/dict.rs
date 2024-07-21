use std::path::Path;

use bitcode::{Decode, Encode};
use itertools::Itertools;
use serde::{Deserialize, Deserializer, Serialize};
use trie_rs::map::Trie;
use type_hash::TypeHash;

type CacheData = Vec<(String, Vec<DictionaryEntry>)>;

#[derive(Serialize, Deserialize, Clone, Debug, Encode, Decode, TypeHash)]
pub struct DictionaryEntry {
    pub simplified: String,
    pub traditional: String,
    #[serde(deserialize_with = "pinyin_deserialize")]
    pub pinyin: Vec<Pinyin>,
    pub translations: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Encode, Decode, TypeHash)]
pub struct Pinyin {
    pub tone: Tone,
    pub syllable: String,
}

#[derive(Serialize, Deserialize, Encode, Decode, Clone, Debug, TypeHash, Copy)]
pub enum Tone {
    First,
    Second,
    Third,
    Fourth,
    Fifth,
    None,
}

impl Tone {
    pub fn from_u8(tone: u8) -> Self {
        match tone {
            1 => Self::First,
            2 => Self::Second,
            3 => Self::Third,
            4 => Self::Fourth,
            5 => Self::Fifth,
            _ => panic!("Invalid tone number"),
        }
    }

    pub fn apply(&self, tone_ch: char) -> char {
        match self {
            Tone::First => match tone_ch {
                'ü' => 'ǖ',
                'a' => 'ā',
                'e' => 'ē',
                'i' => 'ī',
                'o' => 'ō',
                'u' => 'ū',
                _ => tone_ch,
            },
            Tone::Second => match tone_ch {
                'ü' => 'ǘ',
                'a' => 'á',
                'e' => 'é',
                'i' => 'í',
                'o' => 'ó',
                'm' => 'ḿ',
                'u' => 'ú',
                _ => tone_ch,
            },
            Tone::Third => match tone_ch {
                'ü' => 'ǚ',
                'a' => 'ǎ',
                'e' => 'ě',
                'i' => 'ǐ',
                'o' => 'ǒ',
                'u' => 'ǔ',
                _ => tone_ch,
            },
            Tone::Fourth => match tone_ch {
                'ü' => 'ǜ',
                'm' => 'm',
                'a' => 'à',
                'e' => 'è',
                'i' => 'ì',
                'o' => 'ò',
                'u' => 'ù',
                _ => tone_ch,
            },
            _ => tone_ch,
        }
    }
}

pub struct Dictionary {
    data: Trie<u8, Vec<DictionaryEntry>>,
}

impl Dictionary {
    pub fn matches(&self, text: &str) -> Vec<DictionaryEntry> {
        let mut matches = self
            .data
            .common_prefix_search(text)
            .flat_map(|(_, value): (Vec<u8>, &Vec<DictionaryEntry>)| value.clone())
            .collect::<Vec<_>>();
        matches.sort_by_cached_key(|entry| entry.simplified.chars().count());
        matches.reverse();
        matches
    }
}

pub fn load(path: impl AsRef<Path>) -> Dictionary {
    log::info!("Loading data");
    let path = path.as_ref();

    let cache_dir = path.parent().unwrap().join("cache");
    if !cache_dir.exists() {
        std::fs::create_dir_all(&cache_dir).unwrap();
    }
    let cache = cache_dir.join(format!("cedict.{:x}.bin", CacheData::type_hash()));

    let data = if !cache.exists() {
        std::fs::remove_dir_all(&cache_dir).unwrap();

        let data = std::fs::read_to_string(path).unwrap();
        let data: Vec<DictionaryEntry> = serde_json::from_str(&data).unwrap();
        let data = treeify(data);

        // Write cached copy
        let bitcoded = bitcode::encode(&data);
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(cache, bitcoded).unwrap();

        data
    } else {
        let data = std::fs::read(cache).unwrap();
        bitcode::decode(&data).unwrap()
    };
    log::info!("Data loaded. Building tree");
    Dictionary {
        data: Trie::from_iter(data),
    }
}

fn parse_pinyin(pinyin: &str) -> Vec<Pinyin> {
    let syllables = pinyin.trim().split(' ');
    syllables
        .map(|it| {
            if !it.ends_with(['1', '2', '3', '4', '5']) {
                Pinyin {
                    syllable: it.to_string(),
                    tone: Tone::None,
                }
            } else {
                let tone = Tone::from_u8(it.chars().last().unwrap().to_string().parse().unwrap());
                let syllable = normalize_syllable(&it);
                let syllable = apply_tone(&syllable, tone);
                Pinyin { syllable, tone }
            }
        })
        .collect()
}

fn normalize_syllable(syllable: &str) -> String {
    syllable
        .to_lowercase()
        .replacen("u:", "ü", 1)
        .replacen("v", "ü", 1)
        .replacen(char::is_numeric, "", 1)
}

fn apply_tone(syllable: &str, tone: Tone) -> String {
    let vowels = find_vowels(syllable);
    let (tonal_letter_index, tonal_letter) = if vowels.is_empty() {
        syllable.char_indices().next().unwrap()
    } else if vowels.len() == 1 {
        *vowels.first().unwrap()
    } else {
        const PREFERENTIAL_VOWELS: &[char] = &['a', 'e', 'o'];
        if let Some(character) = vowels
            .iter()
            .find(|(_, ch)| PREFERENTIAL_VOWELS.contains(ch))
        {
            *character
        } else {
            vowels.into_iter().nth(1).unwrap()
        }
    };
    let replacement = tone.apply(tonal_letter);
    let mut syllable = syllable.to_owned();
    syllable.replace_range(
        syllable
            .char_indices()
            .nth(tonal_letter_index)
            .map(|(pos, ch)| (pos..pos + ch.len_utf8()))
            .unwrap(),
        &replacement.to_string(),
    );
    syllable
}

fn find_vowels(syllable: &str) -> Vec<(usize, char)> {
    const VOWELS: &[char] = &['a', 'e', 'i', 'o', 'u', 'ü'];
    syllable
        .char_indices()
        .filter(|(_, ch)| VOWELS.contains(ch))
        .collect()
}

fn pinyin_deserialize<'de, D>(deserializer: D) -> Result<Vec<Pinyin>, D::Error>
where
    D: Deserializer<'de>,
{
    let string = Deserialize::deserialize(deserializer)?;
    Ok(parse_pinyin(string))
}

fn treeify(mut data: Vec<DictionaryEntry>) -> CacheData {
    data.sort_by_cached_key(|entry| entry.simplified.to_string());
    let grouped = data
        .into_iter()
        .chunk_by(|entry| entry.simplified.to_string());
    grouped
        .into_iter()
        .map(|(key, entries)| (key, entries.collect()))
        .collect()
}
