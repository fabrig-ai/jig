//! Platform-neutral client trees (client/domain/view.md, sections 3 and 4).
//!
//! The builder owns preorder nodes and an open-parent stack. It knows no
//! domain objects, platform elements or layout. Its calls assert the limits
//! declared by the caller; `finish` requires every opened node to be closed.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod account_catalogue;
mod binding;
pub mod cards;
pub mod confirmation;
pub mod document_text;
mod instructions;
pub mod literal_segment;
pub mod live_call_words;
pub mod notices;
pub mod owner_wake;
pub mod owner_wake_words;
pub mod owner_words;
pub mod pages;
pub mod proposed_task;
pub mod proposed_task_words;
pub mod records;
#[rustfmt::skip]
#[path = "generated/records_v1.rs"]
pub mod records_wire;
pub mod sections;
pub mod specification;
pub mod styles;
mod task_result;
pub mod turns;
pub use styles::JigStyle;
mod chip_phase;
pub mod frame;
mod frame_facts;
pub mod limits;
mod owner;
pub mod phrases;
pub mod queries;
pub mod regions;
mod runtime;
mod sizes;
pub use owner::Owner;
pub mod words;

pub use binding::{JigBindings, JigMeaning, decode, jig_bindings_worst_case};
pub use pages::{after_page, jig};
pub use sizes::{
    arranged_nodes, banner_nodes, card_nodes, chip_nodes, confirmation_nodes, conversation_nodes, conversations_nodes,
    deployment_nodes, escalation_nodes, go_to_task_nodes, inbox_nodes, link_nodes, navigation_nodes, needs_nodes,
    not_the_work_nodes, notices_nodes, offered_nodes, page_nodes, party_nodes, person_task_nodes, project_header_nodes,
    project_nodes, project_section_nodes, projects_nodes, proposal_nodes, question_nodes, refusal_nodes, reply_nodes,
    result_nodes, task_header_nodes, task_nodes, task_section_nodes,
};
pub use turns::conversation;

#[cfg(test)]
mod tests;

pub mod owner_authority;
pub mod owner_authority_words;
pub mod owner_budget;
pub mod owner_budget_words;
pub mod owner_result;
pub mod owner_result_words;
pub mod policy_edit;

pub mod owner_outcome;
pub mod owner_outcome_words;

pub mod owner_chip;
pub mod owner_chip_words;

pub mod owner_amendment;
pub mod owner_amendment_words;

pub mod owner_activity;
pub mod owner_activity_words;

pub mod owner_history;
pub mod owner_history_words;

pub mod owner_milestone;
pub mod owner_milestone_words;

pub mod owner_inbox_words;

pub mod owner_tree;
pub mod owner_tree_words;

pub mod owner_person_task;
pub mod owner_person_task_words;
pub mod owner_proposal;
pub mod owner_proposal_words;

pub mod historical_pages;
pub mod historical_pages_words;
pub mod owner_project_words;
pub use sections::project::owner as owner_project;
pub mod owner_deployment;
pub mod owner_deployment_words;
pub mod owner_historical;
pub mod owner_historical_words;

pub mod action;
pub(crate) mod activity;
pub mod acts;
pub mod address;
pub mod application;
pub mod boundary;
pub(crate) mod clock;
pub mod confirm;
pub mod drafts;
pub mod fields;
pub(crate) mod handles;
pub mod intents;
pub mod objects;
mod outcome;
pub mod page;
pub mod signin;
pub mod store;
pub mod ui;
pub mod windows;
pub use boundary::{Event, How, Refused, Request};
pub use limits::ui::{Limits, valid, worst_case};
pub use ui::{Borrowed, Ui, fire, max_out, next_deadline, reclaim, step};
