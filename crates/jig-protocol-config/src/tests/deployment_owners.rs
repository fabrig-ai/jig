//! First owners hold the deployment's roles (domain/people.md, 3).
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "tests are ordinary Rust (programming-model.md, 10.2)"
)]
use super::{
    domain::{core_config, two_projects},
    fixtures::{ENGINE_SMALL, with},
};
use crate::{Profile, profile};
use jig_domain_core::{Core, CoreRecord, Event, Record, Request, Requests, RestartRequest, RestartStep, Write};
use jig_domain_people as people;
use skein_lib::{Env, ReplyTo, Time, Token, Wall};

const PROJECT: u32 = 1;
const SECOND_PROJECT: u32 = 2;
const OWNER: u64 = 1;
const SECOND_PERSON: u64 = 2;
const ONE_SAVE: usize = 1;

fn saves(core: &mut Core, env: &Env<jig_domain_core::Limits>, event: Event) -> Vec<Record> {
    let Requests::Out(mut out) = jig_domain_core::step(core, env, event);
    let mut records = Vec::new();
    while let Some(request) = out.pop() {
        if let Request::Write(Write::Save(record)) = request {
            records.push(record);
        }
    }
    records
}

struct Row {
    shows: &'static str,
    document: Box<[u8]>,
    restart: bool,
    sign_in: bool,
    roles: Vec<(u64, u32, Option<people::Role>)>,
    deployment_saves: usize,
    deployment_policy: usize,
    deployment_owners: &'static [u64],
}

fn rows() -> [Row; 6] {
    let second_owner = with(
        ENGINE_SMALL,
        br#""subject": "owner-a"}"#,
        br#""subject": "owner-a"}, {"provider": 1, "subject": "aaa-owner"}"#,
    );
    [
        Row {
            shows: "one first owner owns the deployment and its configured project",
            document: ENGINE_SMALL.into(),
            restart: false,
            sign_in: false,
            roles: vec![
                (OWNER, people::DEPLOYMENT, Some(people::Role::Owner)),
                (OWNER, PROJECT, Some(people::Role::Owner)),
            ],
            deployment_saves: ONE_SAVE,
            deployment_policy: 0,
            deployment_owners: &[OWNER],
        },
        Row {
            shows: "Small holds two configured projects and the deployment's roles",
            document: two_projects(b'2'),
            restart: false,
            sign_in: false,
            roles: vec![
                (OWNER, people::DEPLOYMENT, Some(people::Role::Owner)),
                (OWNER, PROJECT, Some(people::Role::Owner)),
                (OWNER, SECOND_PROJECT, Some(people::Role::Owner)),
            ],
            deployment_saves: ONE_SAVE,
            deployment_policy: 0,
            deployment_owners: &[OWNER],
        },
        Row {
            shows: "two first owners own the deployment in configuration order",
            document: second_owner,
            restart: false,
            sign_in: false,
            roles: vec![
                (OWNER, people::DEPLOYMENT, Some(people::Role::Owner)),
                (SECOND_PERSON, people::DEPLOYMENT, Some(people::Role::Owner)),
            ],
            deployment_saves: ONE_SAVE,
            deployment_policy: 0,
            deployment_owners: &[OWNER, SECOND_PERSON],
        },
        Row {
            shows: "without projects the first owner still owns the deployment",
            document: super::fixtures::without_projects(ENGINE_SMALL),
            restart: false,
            sign_in: false,
            roles: vec![(OWNER, people::DEPLOYMENT, Some(people::Role::Owner))],
            deployment_saves: ONE_SAVE,
            deployment_policy: 0,
            deployment_owners: &[OWNER],
        },
        Row {
            shows: "a later sign-in gives no deployment role to a non-owner",
            document: ENGINE_SMALL.into(),
            restart: false,
            sign_in: true,
            roles: vec![(SECOND_PERSON, people::DEPLOYMENT, None)],
            deployment_saves: ONE_SAVE,
            deployment_policy: 0,
            deployment_owners: &[OWNER],
        },
        Row {
            shows: "restart restores the first owner's deployment role without reseeding",
            document: ENGINE_SMALL.into(),
            restart: true,
            sign_in: false,
            roles: vec![(OWNER, people::DEPLOYMENT, Some(people::Role::Owner))],
            deployment_saves: 0,
            deployment_policy: 0,
            deployment_owners: &[],
        },
    ]
}

#[test]
fn first_owners_hold_the_deployments_roles_across_start_and_restart() {
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: profile(Profile::Small).core };
    let mut first_saves = Vec::new();
    for row in rows() {
        let mut core = Core::new(core_config(&row.document), &env.limits).expect("real Small core");
        let _load = core.restart_begin(false);
        if row.restart {
            for record in &first_saves {
                match record {
                    Record::Core(record) => {
                        let _restored = core.restore_core(record.clone(), &env.limits);
                    }
                    Record::People(people::Stored::Policy { project, name, value }) => {
                        let _restored = core.restore_policy(&env.limits, *project, name.clone(), value.clone());
                    }
                    Record::People(record) => {
                        drop(saves(&mut core, &env, Event::People(people::Event::Restore { record: record.clone() })));
                    }
                    Record::Tasks(_) | Record::Accounts(_) | Record::Notes(_) => panic!("first-start records"),
                }
            }
        }
        let mut records = saves(&mut core, &env, Event::People(people::Event::Restored));
        // The root saves the deployment header with the decision's people records.
        if first_saves.is_empty() {
            first_saves.clone_from(&records);
            first_saves.insert(0, Record::Core(CoreRecord::Deployment(core.counters.next_commit())));
        }
        let mut restart = core.restart_done(RestartStep::LoadCore);
        loop {
            match restart {
                RestartRequest::Step(next) => restart = core.restart_done(next),
                RestartRequest::Idle => break,
                RestartRequest::Settings { .. }
                | RestartRequest::HeldPastLimit { .. }
                | RestartRequest::AccountsPastLimit { .. }
                | RestartRequest::ProjectPolicy { .. }
                | RestartRequest::Refused { .. } => panic!("first-owner restart refused: {restart:?}"),
            }
        }
        if row.sign_in {
            records.extend(saves(
                &mut core,
                &env,
                Event::SignIn {
                    held: None,
                    reply_to: ReplyTo::new(Token::new(1)),
                    identity: people::Identity {
                        key: people::IdentityKey { provider: 1, subject: Box::from(&b"later-person"[..]) },
                        login: Box::new([]),
                        name: Box::new([]),
                    },
                },
            ));
            assert_eq!(
                core.people.identity(SECOND_PERSON).unwrap().key.subject.as_ref(),
                b"later-person",
                "{}",
                row.shows
            );
        }
        for &(person, project, expected) in &row.roles {
            assert_eq!(
                core.people.role(person, project),
                expected,
                "{}: person {person}, project {project}",
                row.shows
            );
        }
        let rosters: Vec<_> = records
            .iter()
            .filter_map(|record| match record {
                Record::People(people::Stored::Roles { project, holdings }) if *project == people::DEPLOYMENT => {
                    Some(holdings.as_ref())
                }
                Record::Core(_) | Record::People(_) | Record::Tasks(_) | Record::Accounts(_) | Record::Notes(_) => None,
            })
            .collect();
        assert_eq!(rosters.len(), row.deployment_saves, "{}", row.shows);
        let holdings: Vec<_> = rosters.iter().flat_map(|holdings| holdings.iter().copied()).collect();
        let expected: Vec<_> =
            row.deployment_owners.iter().map(|&person| people::Holding { person, role: people::Role::Owner }).collect();
        assert_eq!(holdings, expected, "{}", row.shows);
        let mut policies = 0;
        for record in &records {
            if let Record::People(people::Stored::Policy { project, .. }) = record
                && *project == people::DEPLOYMENT
            {
                policies += 1;
            }
        }
        assert_eq!(policies, row.deployment_policy, "{}", row.shows);
    }
}
