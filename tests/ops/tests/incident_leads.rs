use jig_ops_domain_observability as obs;
use skein_lib::{Env, Queue, Time, Wall};

fn step(domain: &mut obs::Domain, env: &Env<obs::Limits>, input: obs::Event, out: &mut Queue<obs::Request>) {
    obs::step(domain, env, input, out);
    while out.pop().is_some() {}
}

#[test]
fn full_retained_leads_alert_accounts_and_maximum_templates_keep_their_associations() {
    let limits = jig_ops_world::limits();
    let templates = (0..limits.watches)
        .map(|number| obs::TriageTemplate {
            number: u16::try_from(number).expect("bounded fixture"),
            delegate: vec![b't'; usize::try_from(limits.answer_bytes).expect("bounded fixture")].into(),
        })
        .collect();
    let leads = (0..limits.watches)
        .map(|number| obs::LeadTemplate {
            number: u16::try_from(number).expect("bounded fixture"),
            delegate: vec![b'i'; usize::try_from(limits.answer_bytes).expect("bounded fixture")].into(),
        })
        .collect();
    let mut domain = obs::Domain::with_templates(&limits, templates).with_leads(&limits, leads);
    let mut out = Queue::with_capacity(obs::MAX_OUT);
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    fill_incidents(&mut domain, &env, &mut out);
    for watch in 0..limits.watches {
        let task = u64::from(watch) + 1;
        let service = service(&limits, watch);
        for observed in 0..=limits.samples_per_service {
            step(
                &mut domain,
                &env,
                obs::Event::System(obs::SystemEvent::Fact {
                    service: service.clone(),
                    fact: obs::Fact {
                        observed: u64::from(observed),
                        healthy_replicas: 1,
                        errors: 1,
                        error_rate_percent: 3,
                        load_percent: 20,
                        load: Box::new([]),
                    },
                }),
                &mut out,
            );
        }
        for offset in 0..limits.alerts_per_batch {
            let alert = u64::from(watch * limits.alerts_per_batch + offset) + 1;
            step(
                &mut domain,
                &env,
                obs::Event::System(obs::SystemEvent::Alert {
                    own: false,
                    alert: obs::Alert { number: alert, service: service.clone(), severity: 3 },
                }),
                &mut out,
            );
        }
        step(
            &mut domain,
            &env,
            obs::Event::WatchStep { task, step: 99, batch: 999, messages: Box::new([obs::WatchMessage::News]) },
            &mut out,
        );
    }
    let read = skein_lib::Token::new(990);
    step(
        &mut domain,
        &env,
        obs::Event::DocumentPrepare { token: read, subject: obs::ClientSubject::Project { project: 1 } },
        &mut out,
    );
    obs::step(
        &mut domain,
        &env,
        obs::Event::ReadDocument { scope: obs::ClientScope::Project(1), read, party: 1 },
        &mut out,
    );
    let Some(obs::Request::ReadAnswered { answer: Ok(obs::ClientDocument::Project { incidents, .. }), .. }) = out.pop()
    else {
        panic!("whole incident table")
    };
    assert_eq!(incidents.len(), limits.watches * limits.alerts_per_batch);
    assert!(incidents.iter().all(|incident| incident.lead.is_some() && incident.cut));
    drop(incidents);
    for task in 1..=u64::from(limits.watches) {
        step(&mut domain, &env, obs::Event::StopWatch { task }, &mut out);
    }
}

fn service(limits: &obs::Limits, watch: u32) -> obs::Service {
    let mut name = vec![b's'; usize::try_from(limits.name_bytes).expect("bounded fixture")];
    *name.last_mut().expect("bounded fixture") = b's' + u8::try_from(watch).expect("bounded fixture");
    obs::Service::new(vec![b'e'; usize::try_from(limits.name_bytes).expect("bounded fixture")].into(), name.into())
}

fn fill_incidents(domain: &mut obs::Domain, env: &Env<obs::Limits>, out: &mut Queue<obs::Request>) {
    let limits = env.limits;
    for watch in 0..limits.watches {
        let task = u64::from(watch) + 1;
        let service = service(&limits, watch);
        step(
            domain,
            env,
            obs::Event::StartWatch {
                task,
                project: 1,
                services: Box::new([service.clone()]),
                template: u16::try_from(watch).expect("bounded fixture"),
            },
            out,
        );
        for offset in 0..limits.alerts_per_batch {
            let alert = u64::from(watch * limits.alerts_per_batch + offset) + 1;
            let triage = 100 + alert;
            let lead = 200 + alert;
            let selected = u64::from(offset) * 2 + 1;
            step(
                domain,
                env,
                obs::Event::System(obs::SystemEvent::Alert {
                    own: false,
                    alert: obs::Alert { number: alert, service: service.clone(), severity: 3 },
                }),
                out,
            );
            step(
                domain,
                env,
                obs::Event::WatchStep {
                    task,
                    step: selected,
                    batch: triage,
                    messages: Box::new([obs::WatchMessage::News]),
                },
                out,
            );
            step(domain, env, obs::Event::ProcedureDelegateOutcome { task, step: selected, child: Some(triage) }, out);
            step(
                domain,
                env,
                obs::Event::WatchStep {
                    task,
                    step: selected + 1,
                    batch: lead,
                    messages: Box::new([obs::WatchMessage::Triage {
                        child: triage,
                        verdict: obs::TriageVerdict::Incident {
                            alerts: Box::new([alert]),
                            service: service.clone(),
                            severity: 3,
                        },
                    }]),
                },
                out,
            );
            step(
                domain,
                env,
                obs::Event::ProcedureDelegateOutcome { task, step: selected + 1, child: Some(lead) },
                out,
            );
        }
    }
}
