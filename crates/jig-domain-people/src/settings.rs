//! Setting names and check faults carried by keyed answers (engine.md, 17.1).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Setting {
    Charter(u32),
    Template(u32),
    Policy(u32),
    Rules,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SettingFault {
    Shape,
    Count,
    Reference,
    Rules,
    Budget,
    Grace,
    Owner,
}

/// Whether an accepted settings revision moves running work (engine.md, 17.2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Running {
    Stays,
    Moves,
}
