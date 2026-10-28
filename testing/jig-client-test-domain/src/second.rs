//! The second connector owns resource/kind words and the shared link only.
use jig_client::Link;
use jig_client::contract::Part;
use skein_lib::List;
#[derive(Debug)]
pub struct Side {
    parts: List<Held>,
    link: Link,
    party: Option<u64>,
    me: bool,
    shown: u64,
    text: u32,
    segments: u32,
}
#[derive(Debug)]
struct Held {
    part: Part,
}
impl Side {
    #[must_use]
    pub fn new(capacity: u32, segments: u32, text: u32, _seed: u64) -> Self {
        Self {
            parts: List::with_capacity(capacity),
            link: Link::Starting,
            party: None,
            me: false,
            shown: 0,
            text,
            segments,
        }
    }
    #[must_use]
    pub const fn link(&self) -> Link {
        self.link
    }
    #[must_use]
    pub const fn party(&self) -> Option<u64> {
        self.party
    }
    pub(crate) fn show(&mut self, part: Part) -> bool {
        match &part {
            Part::Resource { path, .. } => {
                if !jig_client::contract::resource_path_fits(path, self.segments, self.text) {
                    return false;
                }
            }
            Part::Connector { .. }
            | Part::Kind { .. }
            | Part::Panel { .. }
            | Part::Result { .. }
            | Part::Effect { .. }
            | Part::OwnerEffect { .. }
            | Part::Outbox { .. }
            | Part::Hold { .. }
            | Part::Project { .. }
            | Part::Deployment { .. } => {}
        }
        for held in &self.parts {
            if held.part == part {
                return true;
            }
        }
        if self.parts.push(Held { part }).is_err() {
            return false;
        }
        self.changed();
        true
    }
    pub(crate) fn left(&mut self) {
        self.parts.clear();
        self.changed();
    }
    fn changed(&mut self) {
        self.shown = self.shown.checked_add(1).expect("shown count fits");
    }
    #[must_use]
    pub const fn shown(&self) -> u64 {
        self.shown
    }
    #[must_use]
    pub fn contains(&self, part: &Part) -> bool {
        for held in &self.parts {
            if &held.part == part {
                return true;
            }
        }
        false
    }
}
impl Side {
    /// Read this connector's own `me` document; no sibling supplies its party.
    pub fn told(
        &mut self,
        client: &jig_client::Client,
        told: &jig_client::Told,
        out: &mut skein_lib::Queue<jig_client::Event>,
    ) {
        let handle = jig_client::Handle { asker: jig_client::Asker(u32::from(crate::SECOND)), number: u64::MAX };
        match told {
            jig_client::Told::Link { link } => {
                self.link = *link;
                match link {
                    Link::Starting | Link::Live | Link::Offline => {
                        if !self.me {
                            self.me = true;
                            out.push(jig_client::Event::Watch {
                                handle,
                                subject: jig_client::Watchable::Jig(jig_client::contract::Subject::Me),
                            });
                        }
                    }
                    Link::SignedOut | Link::OutOfDate => {
                        self.party = None;
                        if self.me {
                            self.me = false;
                            out.push(jig_client::Event::Unwatch { handle });
                        }
                    }
                }
                self.changed();
            }
            jig_client::Told::Changed { handle: value } if *value == handle => {
                if let Some(document) = client.document(handle)
                    && let Some(jig_client::Latest::Jig(jig_client::contract::Delivery::Me(frame))) = document.latest
                {
                    self.party = Some(frame.party.number);
                    self.changed();
                }
            }
            jig_client::Told::Ended { handle: value, .. } if *value == handle => {
                self.party = None;
                self.me = false;
                self.changed();
            }
            jig_client::Told::Changed { .. }
            | jig_client::Told::Ended { .. }
            | jig_client::Told::Page { .. }
            | jig_client::Told::Text { .. }
            | jig_client::Told::Pending { .. }
            | jig_client::Told::Answered { .. }
            | jig_client::Told::Kept { .. }
            | jig_client::Told::Dropped { .. } => {}
        }
    }
}
