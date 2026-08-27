pub(crate) mod boss_battle;
pub(crate) mod word_rain;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GameKind {
    WordRain,
}

impl GameKind {
    pub(crate) const ALL: [Self; 1] = [Self::WordRain];
}
