//! Shared amendment payloads for the tests.
/// An amendment retaining every field, with the supplied explanation.
#[must_use]
pub fn person_unchanged(reason: &[u8]) -> jig_domain_people::Amendment {
    jig_domain_people::Amendment {
        llm: None,
        tracked: None,
        addressed: None,
        instructions: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: None,
        reason: reason.into(),
    }
}

pub fn member(world: &mut crate::World) -> u64 {
    let call = world.signin(10, 1);
    assert!(world.reply(call).is_none());
    world.commit_all();
    let jig_domain_people::Reply::SignedIn { person, .. } =
        world.reply(call).expect("test values are admitted and fit")
    else {
        panic!("signed in")
    };
    world.roles(1, Box::new([jig_domain_people::Holding { person, role: jig_domain_people::Role::Member }]));
    world.commit_all();
    person
}

#[must_use]
pub fn people_member(words: &[u8], spend: u64) -> jig_domain_people::Member {
    use jig_domain_people as people;
    people::Member {
        llm: None,
        deadline: jig_domain_people::Deadline::Named,
        executor: people::TaskExecutor::Agent { charter: 1 },
        words: words.into(),
        parameters: Box::new([]),
        inputs: Box::new([]),
        budget: people::Budget::Named,
        named: Box::new([]),
        contract: people::Contract::Report { words: 16 },
        authority: people::Authority {
            tools: 0,
            models: None,
            grants: Box::new([]),
            delegation: people::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
            spend,
            deadline: None,
            notes: 0,
            note_resources: Box::new([]),
        },
        wake: people::WakePolicy {
            markers: 0,
            words: people::WakeRule::Immediate,
            notices: people::WakeRule::Never,
            news: people::WakeRule::Never,
            results: people::ResultsWake::Never,
            questions: false,
            answers: false,
            timers: false,
        },
        dependencies: Box::new([]),
        tracked: None,
        label: 0,
        flags: people::Flags::PLAIN,
    }
}

#[must_use]
pub fn create(words: &[u8], tracked: Option<u32>) -> jig_domain_people::Ask {
    let mut member = people_member(words, 1);
    member.tracked = tracked;
    jig_domain_people::Ask::Create { project: 1, batch: Box::new([member]) }
}
