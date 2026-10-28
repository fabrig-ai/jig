//! The copy's read-only check before the switch (store.md 10.2, step 5).
use super::plan::Levels;
use crate::codec::component::{Count, CountParts};
use crate::directory::OWNER;
use crate::generations::PAGE;
use crate::{
    Arrived, Below, Check, KeptFamily, KeptMap, OpenRefusal, Owner, Root, Row, Store, intervals, pages, restore,
};
use alloc::boxed::Box;
use jig_domain_core::{Record, RestoreRange};
use skein_io::{file, kernel::Kind};
use skein_lib::{List, Token};

/// B1's manifest order is the supplied sets' order, then their families'.
#[derive(Debug)]
pub(crate) struct Counted {
    pub owner: Owner,
    pub family: KeptFamily,
    pub before: u64,
}

/// Both maps are in memory. Each nonterminal page consumes at least one
/// row; skein-kv's actual row count bounds the pages, plus the empty page.
pub(crate) fn count(store: &mut Store, owner: Owner, family: &KeptFamily) -> Option<u64> {
    let limits = pages::map_limits(store, family.map);
    let rows = match family.map {
        KeptMap::Store => store.stores.store.len(),
        KeptMap::Secrets => store.stores.secrets.len(),
    };
    let mut count = 0_u64;
    for keys in family.keys {
        let range = pages::kept_interval(store, owner, family.map, keys)?;
        let mut cursor = None;
        for _ in 0..=rows {
            let page =
                pages::walk_map(store, family.map, core::slice::from_ref(&range), cursor.as_ref(), limits.page.rows)?;
            if page.rows.is_empty() && page.next.is_some() {
                return None;
            }
            count = count.checked_add(u64::try_from(page.rows.len()).ok()?)?;
            cursor = page.next;
            if cursor.is_none() {
                break;
            }
        }
        if cursor.is_some() {
            return None;
        }
    }
    Some(count)
}

pub(crate) fn before(store: &mut Store) -> Option<List<Counted>> {
    let mut counts = List::with_capacity(crate::component_v3::CEILINGS.manifest_counts);
    for set in store.patch_sets.sets {
        for family in set.families {
            let before = count(store, set.owner, family)?;
            counts
                .push(Counted { owner: set.owner, family: *family, before })
                .expect("plan::bounds checked the families' count before the plan");
        }
    }
    Some(counts)
}

pub(crate) fn counts(store: &mut Store, before: List<Counted>) -> Result<List<Count>, OpenRefusal> {
    let mut counts = List::with_capacity(before.len());
    for counted in before.into_boxed() {
        let after = count(store, counted.owner, &counted.family).ok_or(OpenRefusal::Failed)?;
        if after != counted.before {
            return Err(OpenRefusal::Check {
                why: Check::Count { family: counted.family.name, before: counted.before, after },
            });
        }
        let count = Count::new(
            &crate::component_v3::CEILINGS,
            CountParts { family: Box::from(counted.family.name.as_bytes()), before: counted.before, after },
        )
        .expect("plan::bounds checked each family's name before the plan");
        counts.push(count).expect("one count per kept family");
    }
    Ok(counts)
}

fn ranges() -> Box<[skein_kv::Range]> {
    let mut count = 1_u32;
    for range in &RestoreRange::LIVE {
        count = count
            .checked_add(u32::try_from(intervals::restore(range).len()).expect("fixed live intervals"))
            .expect("fixed live intervals and ended reads");
    }
    let mut ranges = List::with_capacity(count);
    for range in &RestoreRange::LIVE {
        for range in intervals::restore(range) {
            ranges.push(range).expect("counted live intervals");
        }
    }
    ranges.push(intervals::ended_reads()).expect("one ended-read interval");
    ranges.into_boxed()
}

fn rows(store: &mut Store) -> Result<(), OpenRefusal> {
    // Results after parties' read positions and unanswered questions are the
    // core's census selection; its and the children's strict rules come later.
    let ranges = ranges();
    let mut cursor = None;
    let mut join = None;
    for _ in 0..=store.stores.store.len() {
        let page =
            pages::walk(store, &ranges, cursor.as_ref(), store.limits.store.page.rows).ok_or(OpenRefusal::Failed)?;
        if page.rows.is_empty() && page.next.is_some() {
            return Err(OpenRefusal::Failed);
        }
        for item in page.rows {
            let key = item.cursor.after;
            let Some((row, _)) = pages::decode(store, item.row) else {
                return Err(OpenRefusal::Check { why: Check::Load { key } });
            };
            match row {
                Row::Jig(Record::People(jig_domain_people::Stored::EndedRead { task, .. })) => {
                    if restore::ended(store, task).is_none() && join.is_none() {
                        join = Some(Check::Join { key });
                    }
                }
                Row::Jig(_) | Row::Application { .. } => {}
            }
        }
        cursor = page.next;
        if cursor.is_none() {
            // Load precedes Join, even if an unreadable row follows a missing
            // join. One walk serves both, retaining only the first bad join.
            return match join {
                Some(why) => Err(OpenRefusal::Check { why }),
                None => Ok(()),
            };
        }
    }
    Err(OpenRefusal::Failed)
}

#[derive(Debug)]
enum Step {
    Opening,
    Listing,
    File,
    Stat(Token),
    Close,
    Leaving,
    Done,
}

/// One listing page, all its files stated before another page is asked for,
/// and one request in flight on the transcripts driver. Nothing writes.
#[derive(Debug)]
pub(crate) struct Checking {
    before: Levels,
    counts: List<Count>,
    root: Option<Root>,
    entries: Box<[file::Entry]>,
    next: usize,
    last: bool,
    refusal: Option<OpenRefusal>,
    step: Step,
}

impl Checking {
    pub(crate) fn into_counts(self) -> (Levels, List<Count>) {
        (self.before, self.counts)
    }
}

pub(crate) fn start(
    store: &mut Store,
    before: Levels,
    counted: List<Counted>,
    below: &mut Below,
) -> Result<Checking, OpenRefusal> {
    let counts = counts(store, counted)?;
    rows(store)?;
    // The private root belongs to the secrets driver. Open a listing handle
    // beneath its descriptor on the transcripts driver, as the copy does.
    below.transcripts.push(file::Request::OpenDirectory {
        owner: OWNER,
        root: store.directory.private_root.expect("copy's private root is open").fd,
        name: Box::from(b".".as_slice()),
        no_follow: true,
    });
    Ok(Checking {
        before,
        counts,
        root: None,
        entries: Box::new([]),
        next: 0,
        last: false,
        refusal: None,
        step: Step::Opening,
    })
}

fn page(checking: &mut Checking, below: &mut Below) {
    checking.entries = Box::new([]);
    below.transcripts.push(file::Request::ListPage {
        owner: OWNER,
        directory: checking.root.expect("listing root open").token,
        max: PAGE,
    });
    checking.step = Step::Listing;
}
fn leave(checking: &mut Checking, below: &mut Below) {
    below
        .transcripts
        .push(file::Request::Close { owner: OWNER, file: checking.root.expect("listing root open").token });
    checking.step = Step::Leaving;
}
fn next(checking: &mut Checking, below: &mut Below) {
    if checking.refusal.is_some() {
        return leave(checking, below);
    }
    if let Some(entry) = checking.entries.get(checking.next) {
        if entry.kind != Kind::File {
            checking.refusal = Some(OpenRefusal::Check { why: Check::Private { name: entry.name.clone() } });
            return leave(checking, below);
        }
        below.transcripts.push(file::Request::OpenReadNoFollow {
            owner: OWNER,
            root: checking.root.expect("listing root open").fd,
            name: entry.name.clone(),
        });
        checking.step = Step::File;
    } else if checking.last {
        leave(checking, below);
    } else {
        page(checking, below);
    }
}

pub(crate) fn up(checking: &mut Checking, arrived: Arrived, below: &mut Below) -> Result<bool, OpenRefusal> {
    match core::mem::replace(&mut checking.step, Step::Done) {
        Step::Opening => {
            let root = crate::directory::opened(arrived).ok_or(OpenRefusal::Failed)?;
            checking.root = Some(root);
            page(checking, below);
        }
        Step::Listing => {
            let file::Event::Listed { entries, .. } = arrived.event else { return Err(OpenRefusal::Failed) };
            checking.last = entries.len() < usize::try_from(PAGE).expect("PAGE fits usize");
            checking.entries = entries;
            checking.next = 0;
            next(checking, below);
        }
        Step::File => {
            let file::Event::Opened { file, .. } = arrived.event else { return Err(OpenRefusal::Failed) };
            below.transcripts.push(file::Request::Stat { owner: OWNER, file });
            checking.step = Step::Stat(file);
        }
        Step::Stat(file) => {
            let file::Event::Stated { stat, .. } = arrived.event else { return Err(OpenRefusal::Failed) };
            if stat.kind != Kind::File || stat.mode & 0o077 != 0 || stat.links != 1 {
                let name = checking.entries.get(checking.next).expect("stated entry retained").name.clone();
                checking.refusal = Some(OpenRefusal::Check { why: Check::Private { name } });
            }
            below.transcripts.push(file::Request::Close { owner: OWNER, file });
            checking.step = Step::Close;
        }
        Step::Close => {
            let file::Event::Closed { .. } = arrived.event else { return Err(OpenRefusal::Failed) };
            checking.next = checking.next.checked_add(1).expect("listing page bounded by PAGE");
            next(checking, below);
        }
        Step::Leaving => {
            let file::Event::Closed { .. } = arrived.event else { return Err(OpenRefusal::Failed) };
            checking.root = None;
            return match checking.refusal.take() {
                Some(why) => Err(why),
                None => Ok(true),
            };
        }
        Step::Done => return Err(OpenRefusal::Failed),
    }
    Ok(false)
}
