#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(crate) enum BossKind {
    IronWarden,
    ThornQueen,
    NullArchon,
}

impl BossKind {
    pub(crate) const ALL: [Self; 3] = [Self::IronWarden, Self::ThornQueen, Self::NullArchon];

    pub(crate) const fn index(self) -> usize {
        self as usize
    }
}
