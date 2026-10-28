//! Fresh wire names and one terminal per watch (protocol/client.md, 7).
use crate::{Limits, Request};
use jig_client_domain::Family;
use jig_client_domain::contract::{WatchEnd, WatchEvent};
use jig_client_protocol_wire::v19 as w;
use skein_lib::{Map, Queue, Token};
#[derive(Debug)]
pub(crate) struct Opening {
    pub(crate) name: u64,
    pub(crate) owner: w::Owner,
    pub(crate) version: u32,
    pub(crate) subject: Option<w::Owned>,
    pub(crate) live: bool,
    pub(crate) closing: bool,
}
#[derive(Debug)]
pub(crate) struct Watches(pub(crate) Map<Token, Opening>);
impl Watches {
    pub(crate) fn new(limits: &Limits) -> Self {
        Self(Map::with_capacity(limits.client.watches))
    }
    pub(crate) fn named(&self, name: u64) -> Option<Token> {
        for (token, opening) in &self.0 {
            if opening.name == name {
                return Some(*token);
            }
        }
        None
    }
    pub(crate) fn end(&mut self, watch: Token, end: WatchEnd, out: &mut Queue<Request>) {
        if self.0.remove(&watch).is_some() {
            out.push(Request::Ended { watch, end });
        }
    }
    pub(crate) fn opened(&mut self, watch: Token, out: &mut Queue<Request>) {
        if let Some(opening) = self.0.get_mut(&watch)
            && !opening.live
            && !opening.closing
        {
            opening.live = true;
            out.push(Request::Watch { watch, event: WatchEvent::Opened });
        }
    }
    pub(crate) fn alive(&self, out: &mut Queue<Request>) {
        for (watch, opening) in &self.0 {
            if !opening.closing {
                out.push(Request::Watch { watch: *watch, event: WatchEvent::Alive });
            }
        }
    }
    pub(crate) fn delivery(
        &mut self,
        value: w::Delivery,
        limits: &Limits,
        out: &mut Queue<Request>,
    ) -> Result<(), crate::Undecodable> {
        let Some(watch) = self.named(value.watch()) else { return Ok(()) };
        let opening = self.0.get(&watch).expect("named watch");
        if opening.closing {
            return Ok(());
        }
        let record = value.into_parts().document;
        if record.owner() != &opening.owner || record.version() != opening.version {
            return Err(crate::Undecodable::Version);
        }
        match record.owner() {
            w::Owner::Family(owner) => {
                let family = owner.number();
                self.opened(watch, out);
                out.push(Request::FamilyDelivery {
                    watch,
                    record: Family { family, version: record.version(), bytes: record.into_parts().record },
                });
                Ok(())
            }
            w::Owner::Jig => {
                let value = crate::translate::watches::delivery(record.record(), limits)?;
                self.opened(watch, out);
                out.push(Request::Watch { watch, event: WatchEvent::Delivered(value) });
                Ok(())
            }
        }
    }
}
