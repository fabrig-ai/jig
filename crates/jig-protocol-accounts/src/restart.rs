//! A restart names its live sources, then sweeps unnamed Source records once
//! (domain/accounts.md, 3.4; protocol/store.md, 9). Application rows are untouched.
use skein_lib::{Queue, Set};

#[derive(Debug)]
enum Phase {
    Naming,
    Loading { after: Option<u64> },
    Awaiting,
    Erasing,
    Erased,
    Done,
}
/// One bounded store operation; the host translates only the owner and exchange.
#[derive(Debug)]
pub enum Sweep {
    Load { after: Option<u64> },
    Erase { number: u64 },
}
#[derive(Debug)]
pub struct Inventory {
    named: Set<u64>,
    phase: Phase,
    unnamed: Queue<u64>,
    next: Option<u64>,
}
impl Inventory {
    #[must_use]
    pub fn new(sources: u32, page_rows: u32) -> Self {
        Self {
            named: Set::with_capacity(sources),
            phase: Phase::Naming,
            unnamed: Queue::with_capacity(page_rows),
            next: None,
        }
    }
    /// All restored or newly admitted signed-in and key sources, for every holder.
    pub fn name(&mut self, source: u32) {
        self.named.insert(u64::from(source)).expect("live sources fit driver bound");
    }
    pub fn forget(&mut self, source: u32) {
        let _removed = self.named.remove(&u64::from(source));
    }
    pub fn restored(&mut self) {
        match self.phase {
            Phase::Naming => self.phase = Phase::Loading { after: None },
            Phase::Loading { .. } | Phase::Awaiting | Phase::Erasing | Phase::Erased | Phase::Done => (),
        }
    }
    pub fn row(&mut self, number: u64) {
        if !self.named.contains(&number) {
            self.unnamed.push(number);
        }
    }
    pub fn loaded(&mut self, next: Option<u64>) {
        self.next = next;
        self.phase = Phase::Erased;
    }
    pub fn erased(&mut self) {
        self.phase = Phase::Erased;
    }
    pub fn step(&mut self) -> Option<Sweep> {
        match self.phase {
            Phase::Loading { after } => {
                self.phase = Phase::Awaiting;
                Some(Sweep::Load { after })
            }
            Phase::Erased => {
                for _ in 0..self.unnamed.len() {
                    let number = self.unnamed.pop().expect("unnamed Source rows");
                    // A new LLM key may have been admitted since this page was read.
                    if !self.named.contains(&number) {
                        self.phase = Phase::Erasing;
                        return Some(Sweep::Erase { number });
                    }
                }
                self.phase = match self.next.take() {
                    Some(after) => Phase::Loading { after: Some(after) },
                    None => Phase::Done,
                };
                None
            }
            Phase::Naming | Phase::Awaiting | Phase::Erasing | Phase::Done => None,
        }
    }
    #[must_use]
    pub fn active(&self) -> bool {
        match self.phase {
            Phase::Naming | Phase::Done => false,
            Phase::Loading { .. } | Phase::Awaiting | Phase::Erasing | Phase::Erased => true,
        }
    }
    #[must_use]
    pub fn work_pending(&self) -> bool {
        match self.phase {
            Phase::Loading { .. } | Phase::Erased => true,
            Phase::Naming | Phase::Awaiting | Phase::Erasing | Phase::Done => false,
        }
    }
}
