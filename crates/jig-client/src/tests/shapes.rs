use super::fixtures::*;
use crate::{contract::*, shapes::*};
use skein_lib::{List, Wall};
const CHAT_TASK: u64 = 41;
const CANCELLED: u64 = 43;
const MOVED: u64 = 47;
const DEADLINE: Wall = Wall::from_nanos(100);
fn words() -> Text {
    Text::new(b"Hello".as_slice().into(), 32).unwrap()
}
fn draft() -> GoalDraft {
    GoalDraft {
        words: words(),
        executor: OwnerExecutor::Agent { charter: 1 },
        budget: 150,
        deadline: None,
        priority: Some(7),
        llm: None,
    }
}
#[test]
fn chat_fills_the_chat_template_into_one_as_allowed_task() {
    let me = me();
    let Ask::Create { project, tasks } = chat(&me, PROJECT, words()).unwrap() else { panic!("create") };
    assert_eq!(project, PROJECT);
    assert_eq!(tasks.len(), 1);
    let task = tasks.get(0).unwrap();
    assert_eq!(task.executor, OwnerExecutor::Agent { charter: 1 });
    assert_eq!(
        task.spec,
        Specification {
            asks: false,
            words: words(),
            parameters: List::with_capacity(0),
            inputs: List::with_capacity(0)
        }
    );
    assert_eq!(task.contract, NewContract::Report { words: 32 });
    assert_eq!(task.budget, NewBudget::AsAllowed { cap: None });
    let mut expected = creating().within.authority.whole().unwrap();
    expected.spend = 0;
    expected.deadline = None;
    assert_eq!(task.authority, expected);
    assert_eq!(task.wake, None);
    assert_eq!(task.dependencies.len(), 0);
    assert_eq!(task.named.len(), 0);
    assert_eq!(task.tracked, None);
    assert_eq!(task.label, 1);
    assert_eq!(
        task.flags,
        TaskFlags { ended_by: EndedBy::Result, conversation: true, listed: false, retention: Retention::Kept }
    );
}
#[test]
fn close_chat_cancels_the_chat_last_with_its_template_s_grace() {
    let delegates = [Delegate { task: CANCELLED, fate: Fate::Cancel }, Delegate { task: MOVED, fate: Fate::Move }];
    let Ask::Compound { project, operations } = close_chat(&me(), PROJECT, CHAT_TASK, &delegates, words()).unwrap()
    else {
        panic!("compound")
    };
    assert_eq!(project, PROJECT);
    assert_eq!(
        operations.as_slice(),
        &[
            Operation::Cancel { project: PROJECT, task: CANCELLED, reason: words(), grace: None },
            Operation::Move { project: PROJECT, task: MOVED, reason: words() },
            Operation::Cancel { project: PROJECT, task: CHAT_TASK, reason: words(), grace: Some(SHAPE_GRACE) },
        ]
    );
}
#[test]
fn fall_puts_a_draft_without_a_deadline_beyond_a_bound_that_has_one() {
    let mut creating = creating();
    creating.within.latest = Some(DEADLINE);
    let mut draft = draft();
    draft.budget = 10;
    assert_eq!(
        fall(&creating, &draft, NewDeadline::Named),
        Fall::Proposable(Recipient::Role { project: PROJECT, role: OWNER_ROLE })
    );
    creating.within.maximum_deadline = Some(DEADLINE);
    assert_eq!(fall(&creating, &draft, NewDeadline::Named), Fall::NotOffered(NotOffered::Ceiling));
}
#[test]
fn fall_gives_not_offered_for_an_executor_the_bounds_do_not_list() {
    let mut draft = draft();
    draft.executor = OwnerExecutor::Agent { charter: 2 };
    assert_eq!(fall(&creating(), &draft, NewDeadline::Named), Fall::NotOffered(NotOffered::Executor));
}
#[test]
fn set_goal_proposes_to_the_holder_the_bounds_name() {
    let Judged::Propose { ask, to } = set_goal(&me(), PROJECT, draft(), words()) else { panic!("proposed") };
    assert_eq!(to, Recipient::Role { project: PROJECT, role: OWNER_ROLE });
    let Ask::Propose { project, action: ProposeAction::Batch { tasks }, reason } = ask else { panic!("batch") };
    assert_eq!((project, reason), (PROJECT, words()));
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks.get(0).unwrap().tracked, Some(7));
    assert_eq!(tasks.get(0).unwrap().authority.spend, 150);
}
#[test]
fn a_template_shapes_cannot_fill_is_not_offered() {
    let base = me().projects.get(0).unwrap().templates.get(0).unwrap().clone();
    let cases = [
        ("approval", Template { contract: facts_task().result_contract, ..base.clone() }),
        (
            "cut verdict",
            Template {
                contract: TaskContractDocument::Verdict { choices: List::with_capacity(0), cut: true },
                ..base.clone()
            },
        ),
        ("zero depth", Template { shallower: 0, ..base.clone() }),
        ("executor", Template { executor: TemplateExecutor::Chosen, ..base.clone() }),
        ("budget", Template { budget: TemplateBudget::Given, ..base.clone() }),
        ("deadline", Template { deadline: TemplateDeadline::Given, ..base }),
    ];
    for (name, template) in cases {
        let mut me = me();
        *shape_project(&mut me).templates.get_mut(0).unwrap() = template;
        assert_eq!(chat(&me, PROJECT, words()), Err(NotOffered::Template), "{name}");
    }
}
#[test]
fn a_template_span_is_kept_in_the_ask() {
    let mut me = me();
    shape_project(&mut me).templates.get_mut(0).unwrap().deadline = TemplateDeadline::Span(SHAPE_GRACE);
    let task = shape_task(chat(&me, PROJECT, words()).unwrap());
    assert_eq!(task.deadline, NewDeadline::Span(SHAPE_GRACE));
    assert_eq!(task.authority.deadline, None);
}
#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
fn choice(model: &[u8], effort: Option<&[u8]>) -> LlmChoice {
    let mut subagents = List::with_capacity(1);
    subagents.push(ModelEffort { model: Text::new(b"quick".as_slice().into(), 32).unwrap(), effort: None }).unwrap();
    LlmChoice {
        account: None,
        main: ModelEffort {
            model: Text::new(model.into(), 32).unwrap(),
            effort: match effort {
                Some(effort) => Some(Text::new(effort.into(), 32).unwrap()),
                None => None,
            },
        },
        subagents,
    }
}
#[test]
fn a_template_s_llm_choice_is_copied_into_the_create() {
    let mut me = me();
    shape_project(&mut me).templates.get_mut(0).unwrap().llm = Some(choice(b"careful", Some(b"high")));
    let task = shape_task(chat(&me, PROJECT, words()).unwrap());
    assert_eq!(task.llm, Some(choice(b"careful", Some(b"high"))));
    shape_project(&mut me).templates.get_mut(0).unwrap().llm = None;
    assert_eq!(shape_task(chat(&me, PROJECT, words()).unwrap()).llm, None);
}
#[test]
fn a_goal_draft_s_llm_choice_overrides_its_template_s() {
    let mut me = me();
    shape_project(&mut me).templates.get_mut(1).unwrap().llm = Some(choice(b"careful", Some(b"high")));
    let mut plain = draft();
    plain.budget = 10;
    let Judged::Create(ask) = set_goal(&me, PROJECT, plain, words()) else { panic!("created") };
    assert_eq!(shape_task(ask).llm, Some(choice(b"careful", Some(b"high"))));
    let mut naming = draft();
    naming.budget = 10;
    naming.llm = Some(choice(b"quick", None));
    let Judged::Create(ask) = set_goal(&me, PROJECT, naming, words()) else { panic!("created") };
    assert_eq!(shape_task(ask).llm, Some(choice(b"quick", None)));
}
#[test]
fn a_template_narrows_the_task_delegation() {
    let mut me = me();
    shape_project(&mut me).templates.get_mut(0).unwrap().shallower = SHAPE_NARROWING;
    let task = shape_task(chat(&me, PROJECT, words()).unwrap());
    assert_eq!(task.authority.delegation.depth, SHAPE_DEPTH - (SHAPE_NARROWING - 1));
}
#[test]
fn the_same_inputs_and_me_make_the_same_ask() {
    let mut me = me();
    shape_project(&mut me).templates.get_mut(0).unwrap().deadline = TemplateDeadline::Span(SHAPE_GRACE);
    assert_eq!(chat(&me, PROJECT, words()), chat(&me, PROJECT, words()));
}
#[test]
fn a_given_deadline_remains_a_wall() {
    let mut me = me();
    shape_project(&mut me).templates.get_mut(1).unwrap().deadline = TemplateDeadline::Given;
    let mut draft = draft();
    draft.budget = 10;
    draft.deadline = Some(DEADLINE);
    let Judged::Create(ask) = set_goal(&me, PROJECT, draft, words()) else { panic!("created") };
    let task = shape_task(ask);
    assert_eq!(task.deadline, NewDeadline::Named);
    assert_eq!(task.authority.deadline, Some(DEADLINE));
}
#[test]
fn a_goal_span_is_within_both_finite_deadline_bounds() {
    let me = span_goal_me();
    let mut draft = draft();
    draft.budget = 10;
    let Judged::Create(ask) = set_goal(&me, PROJECT, draft, words()) else { panic!("created") };
    let task = shape_task(ask);
    assert_eq!(task.deadline, NewDeadline::Span(SHAPE_GRACE));
    assert_eq!(task.authority.deadline, None);
}
#[test]
fn a_goal_span_keeps_the_other_creating_judgments() {
    let base = span_goal_me();
    let project = base.projects.get(0).unwrap();
    let creating = project.creating.as_ref().unwrap();
    let mut refused = creating.clone();
    refused.beyond = Beyond::Refused(Refusal::NotOffered);
    let mut cut = creating.clone();
    cut.within.authority.cut = true;
    let cases = [
        ("at budget", 100, 1, Some(creating.clone()), project.templates.clone(), Fall::Within),
        (
            "over budget",
            101,
            1,
            Some(creating.clone()),
            project.templates.clone(),
            Fall::Proposable(Recipient::Role { project: PROJECT, role: OWNER_ROLE }),
        ),
        (
            "at ceiling",
            500,
            1,
            Some(creating.clone()),
            project.templates.clone(),
            Fall::Proposable(Recipient::Role { project: PROJECT, role: OWNER_ROLE }),
        ),
        (
            "over ceiling",
            501,
            1,
            Some(creating.clone()),
            project.templates.clone(),
            Fall::NotOffered(NotOffered::Ceiling),
        ),
        ("executor", 10, 2, Some(creating.clone()), project.templates.clone(), Fall::NotOffered(NotOffered::Executor)),
        ("refused", 101, 1, Some(refused), project.templates.clone(), Fall::Refused(Refusal::NotOffered)),
        ("cut authority", 10, 1, Some(cut), project.templates.clone(), Fall::NotOffered(NotOffered::Creating)),
        ("no creating", 10, 1, None, project.templates.clone(), Fall::NotOffered(NotOffered::Creating)),
        ("no template", 10, 1, Some(creating.clone()), List::with_capacity(0), Fall::NotOffered(NotOffered::Template)),
    ];
    for (name, budget, charter, creating, templates, expected) in cases {
        let mut me = base.clone();
        let project = shape_project(&mut me);
        project.creating = creating;
        project.templates = templates;
        let mut draft = draft();
        draft.budget = budget;
        draft.executor = OwnerExecutor::Agent { charter };
        let judgment = match set_goal(&me, PROJECT, draft, words()) {
            Judged::Create(_) => Fall::Within,
            Judged::Propose { to, .. } => Fall::Proposable(to),
            Judged::Refused(why) => Fall::Refused(why),
            Judged::NotOffered(why) => Fall::NotOffered(why),
        };
        assert_eq!(judgment, expected, "{name}");
    }
}

#[test]
fn close_chat_without_an_offered_template_has_no_grace() {
    let mut me = me();
    shape_project(&mut me).templates = List::with_capacity(0);
    let Ask::Compound { operations, .. } = close_chat(&me, PROJECT, CHAT_TASK, &[], words()).unwrap() else {
        panic!("compound")
    };
    assert_eq!(
        operations.as_slice(),
        &[Operation::Cancel { project: PROJECT, task: CHAT_TASK, reason: words(), grace: None }]
    );
}
#[test]
fn close_chat_reads_the_edited_template_grace() {
    let mut me = me();
    let grace = skein_lib::Duration::from_nanos(1234);
    shape_project(&mut me).templates.get_mut(0).unwrap().grace = Some(grace);
    let Ask::Compound { operations, .. } = close_chat(&me, PROJECT, CHAT_TASK, &[], words()).unwrap() else {
        panic!("compound")
    };
    assert_eq!(
        operations.as_slice(),
        &[Operation::Cancel { project: PROJECT, task: CHAT_TASK, reason: words(), grace: Some(grace) }]
    );
}
