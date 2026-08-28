use crate::model::Difficulty;

pub(crate) mod boss_battle;
pub(crate) mod word_rain;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(crate) enum GameDifficulty {
    Easy,
    Medium,
    Hard,
    Hell,
}

impl GameDifficulty {
    pub(crate) const ALL: [Self; 4] = [Self::Easy, Self::Medium, Self::Hard, Self::Hell];

    pub(crate) const fn index(self) -> usize {
        self as usize
    }

    pub(crate) const fn content_difficulty(self) -> Difficulty {
        match self {
            Self::Easy => Difficulty::Easy,
            Self::Medium => Difficulty::Medium,
            Self::Hard | Self::Hell => Difficulty::Hard,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GameKind {
    WordRain,
    BossBattle,
}

impl GameKind {
    pub(crate) const ALL: [Self; 2] = [Self::WordRain, Self::BossBattle];
}
