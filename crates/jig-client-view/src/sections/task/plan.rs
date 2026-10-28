//! Bounded iterative tree presentation, with held counts on folds (pages.md, 5.2).
use crate::phrases::jig::JigPhrase;
use crate::words::Words;
use crate::{JigBindings, cards};
use skein_ui::phrases::Hole;
use skein_ui::{Builder, Role, State};

use crate::{Borrowed, action::Act, page::task::PlanFilter};
use jig_client::contract::{ObjectKey, TreeNode};
use skein_lib::{Set, Stack};
#[derive(Clone, Copy, Debug)]
enum Visit {
    Enter(u32),
    Leave,
}
pub(super) fn plan(b: &mut Builder, client: &Borrowed<'_>, words: &Words, bindings: &mut JigBindings) {
    let task = client.task_page();
    let plan = &task.plan;
    for (filter, phrase) in [
        (PlanFilter::All, JigPhrase::AllPlan),
        (PlanFilter::Held, JigPhrase::HeldPlan),
        (PlanFilter::Live, JigPhrase::LivePlan),
    ] {
        crate::pages::work::control(b, words, bindings, phrase, Act::FilterPlan { filter }, plan.filter == filter);
    }
    if let Some(owner) = &plan.owner {
        crate::owner_tree::render(b, owner, plan, client, words, bindings);
        crate::pages::cut(b, words, task.ended_cut);
        crate::pages::work::cursors(
            b,
            words,
            bindings,
            task.ended_before.as_ref(),
            task.ended_after.as_ref(),
            crate::page::task::ENDED_WINDOW,
        );
        owner_ended(b, client, words, bindings);
        return;
    }
    b.open(Role::Tree);
    b.name(&words.say(JigPhrase::Plan, &[]));
    let count = u32::try_from(plan.nodes.len()).expect("bounded tree rows fit u32");
    let mut stack = Stack::with_capacity(count.checked_mul(2).expect("bounded tree visits"));
    let mut seen = Set::with_capacity(count);
    for index in 0..count {
        let index = count.checked_sub(1).expect("a row exists").checked_sub(index).expect("reverse row exists");
        let node = plan.nodes.get(usize::try_from(index).expect("row index fits usize")).expect("node exists");
        let mut parent_present = false;
        for parent in plan.nodes {
            if node.parent == Some(parent.chip.task) && plan.visible(parent) {
                parent_present = true;
            }
        }
        if !parent_present && plan.visible(node) {
            stack.push(Visit::Enter(index)).expect("root visit fits");
        }
    }
    for _ in 0..count.checked_mul(2).expect("bounded tree visits") {
        let Some(visit) = stack.pop() else {
            break;
        };
        match visit {
            Visit::Leave => b.close(),
            Visit::Enter(index) => {
                let node =
                    plan.nodes.get(usize::try_from(index).expect("row index fits usize")).expect("named row exists");
                if seen.contains(&node.chip.task) {
                    continue;
                }
                seen.insert(node.chip.task).expect("each row once");
                let expanded = plan.expanded(node);
                item(b, client, node, expanded, words, bindings);
                stack.push(Visit::Leave).expect("paired close fits");
                if expanded {
                    for child in 0..count {
                        let child =
                            count.checked_sub(1).expect("row exists").checked_sub(child).expect("reverse child exists");
                        let next = plan
                            .nodes
                            .get(usize::try_from(child).expect("row index fits usize"))
                            .expect("child exists");
                        if next.parent == Some(node.chip.task) && plan.visible(next) && !seen.contains(&next.chip.task)
                        {
                            stack.push(Visit::Enter(child)).expect("bounded child visit fits");
                        }
                    }
                }
            }
        }
    }
    assert!(stack.is_empty(), "every emitted tree item closes");
    b.close();
    crate::pages::work::counts(b, plan.folded, words);
    words.text(b, JigPhrase::HeldBelow, &[Hole::Number(u64::from(plan.held_below))]);
    crate::pages::cut(b, words, task.ended_cut);
    crate::pages::work::cursors(
        b,
        words,
        bindings,
        task.ended_before.as_ref(),
        task.ended_after.as_ref(),
        crate::page::task::ENDED_WINDOW,
    );
    owner_ended(b, client, words, bindings);
}
fn item(
    b: &mut Builder,
    client: &Borrowed<'_>,
    node: &TreeNode,
    expanded: bool,
    words: &Words,
    bindings: &mut JigBindings,
) {
    let objects = client.objects();
    b.open(Role::TreeItem);
    b.key(crate::pages::object_key(ObjectKey::Task(node.chip.task)));
    b.name(
        &words.say(JigPhrase::ChipNumber, &[Hole::Number(node.chip.task), crate::words::name(node.chip.title.bytes())]),
    );
    if expanded {
        b.state(State::Expanded);
    }
    cards::chip::chip(b, client, &node.chip, words, bindings);
    words.amount(b, node.spend);
    crate::pages::work::counts(b, node.folded, words);
    words.text(b, JigPhrase::HeldBelow, &[Hole::Number(u64::from(node.held_below))]);
    if let Some(id) = objects.find(ObjectKey::Task(node.chip.task)) {
        crate::pages::work::control(
            b,
            words,
            bindings,
            if expanded { JigPhrase::FoldSubtree } else { JigPhrase::OpenSubtree },
            Act::Fold { object: id.token(), open: !expanded },
            expanded,
        );
        if expanded {
            cards::card(b, client, id, &cards::Place::TaskPage, words, bindings);
        }
    }
    if !crate::page::conversations::live(node.chip.phase) {
        crate::frame::control(
            b,
            words,
            bindings,
            Role::Button,
            JigPhrase::ReadEarlier,
            &[],
            Act::OpenEnded { task: node.chip.task },
        );
    }
    if expanded {
        for proposal in &node.proposals {
            if let Some(id) = objects.find(ObjectKey::Proposal(*proposal)) {
                cards::card(b, client, id, &cards::Place::TaskPage, words, bindings);
            }
        }
        for task in &node.dependencies {
            crate::frame::control(
                b,
                words,
                bindings,
                Role::Link,
                JigPhrase::TaskPage,
                &[Hole::Number(*task)],
                Act::Go { address: crate::address::Address::task(*task) },
            );
        }
    }
}

fn owner_ended(b: &mut Builder, client: &Borrowed<'_>, words: &Words, bindings: &mut JigBindings) {
    if let Some(page) = &client.task_page().owner_ended {
        crate::historical_pages::ended_render(b, client, page, words, bindings);
    }
}
