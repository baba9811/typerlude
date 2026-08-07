use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Ko,
    #[default]
    En,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    #[default]
    Mixed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticeKind {
    Quick,
    Key,
    Words,
    Sentence,
    Long,
    Test,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeedUnit {
    Kpm,
    Cpm,
    Wpm,
}
