//! The real smith host half for source commands (hosts.md, 5.1 and 6.8).
//! Paths and a whole environment are injected below the worker root; no login
//! contents or process environment are read here. Commands settle before reuse.
use alloc::boxed::Box;
use jig_domain_host::{self as jig, sources};
use skein_io::{self as io, kernel::Fd};
use skein_lib::{Queue, Time, Token};
use smith_host_domain as smith;
use smith_host_protocol::{ProgramSource, SourceAnswer, SourceAsk};
use smith_protocol_program::Programs;

const PATH_BYTES: usize = 1024;
const ENVIRONMENT_ENTRIES: u32 = 64;
const ENVIRONMENT_BYTES: u64 = 4096;

#[derive(Debug)]
pub struct Resources {
    programs: Programs,
    environment: Box<[Box<[u8]>]>,
}
impl Resources {
    /// Startup's already parsed agent paths and explicit environment.
    #[must_use]
    pub fn from_paths(directory: Box<[u8]>, claude: Box<[u8]>, environment: Box<[Box<[u8]>]>) -> Option<Self> {
        Self::new(Programs { directory, claude }, environment)
    }
    #[must_use]
    pub fn new(programs: Programs, environment: Box<[Box<[u8]>]>) -> Option<Self> {
        if programs.directory.len() > PATH_BYTES
            || programs.claude.is_empty()
            || programs.claude.len() > PATH_BYTES
            || programs.claude.contains(&0)
        {
            return None;
        }
        let configuration = smith_protocol_program::configuration_directory(&programs, &[b'x'; 32]).ok()?;
        // Validate both the retained base and the filtered child environment.
        if environment.len() > usize::try_from(ENVIRONMENT_ENTRIES).ok()? {
            return None;
        }
        let mut bytes = 0_usize;
        for entry in &environment {
            bytes = bytes.checked_add(entry.len())?;
        }
        if u64::try_from(bytes).ok()? > ENVIRONMENT_BYTES {
            return None;
        }
        smith_protocol_program::environment(&environment, &configuration).ok()?;
        Some(Self { programs, environment })
    }
}
#[derive(Debug)]
struct Command {
    owner: Token,
    command: ProgramSource,
}
#[derive(Debug)]
pub struct Component {
    resources: Option<Resources>,
    command: Option<Command>,
    detail: u32,
}
impl Component {
    #[must_use]
    pub fn new(detail: u32) -> Self {
        Self { resources: None, command: None, detail }
    }
    pub fn configure(&mut self, resources: Resources) -> Result<(), Resources> {
        if self.command.is_some() {
            return Err(resources);
        }
        self.resources = Some(resources);
        Ok(())
    }
    #[must_use]
    pub fn owns(&self, owner: Token) -> bool {
        match &self.command {
            Some(command) => command.command.owns(owner),
            None => false,
        }
    }
    #[must_use]
    pub fn settled(&self) -> bool {
        self.command.is_none()
    }
    #[expect(clippy::too_many_arguments, reason = "one typed host command and its explicit I/O binding")]
    pub fn start(
        &mut self,
        owner: Token,
        io_owner: Token,
        name: jig::SourceName,
        operation: sources::Operation,
        root: Fd,
        deadline: Time,
        above: &mut Queue<jig::Event>,
        below: &mut Queue<io::Request>,
    ) {
        assert!(self.command.is_none(), "host serializes source commands");
        let Some(resources) = &self.resources else {
            above.push(failed(
                owner,
                operation,
                jig::SourceFailure::Unconfigured,
                b"no program configured",
                self.detail,
            ));
            return;
        };
        let ask = match operation {
            sources::Operation::Check => SourceAsk::Check,
            sources::Operation::SignOut => SourceAsk::SignOut,
        };
        let source = smith::SourceName::new(name.as_bytes()).expect("same source name grammar");
        match ProgramSource::new(
            io_owner,
            ask,
            &resources.programs,
            source,
            &resources.environment,
            root,
            Box::from(b".".as_slice()),
            deadline,
            self.detail,
            below,
        ) {
            Ok(command) => self.command = Some(Command { owner, command }),
            Err(error) => {
                let detail = match error {
                    smith_protocol_program::Error::Name => b"invalid source name".as_slice(),
                    smith_protocol_program::Error::Directory => b"invalid programs directory".as_slice(),
                    smith_protocol_program::Error::Environment => b"invalid program environment".as_slice(),
                };
                above.push(failed(owner, operation, jig::SourceFailure::Unconfigured, detail, self.detail));
            }
        }
    }
    pub fn from_io(&mut self, event: io::Event, above: &mut Queue<jig::Event>, below: &mut Queue<io::Request>) {
        self.command.as_mut().expect("owned source event").command.from_io(event, below);
        self.answer(above);
    }
    pub fn fire(&mut self, now: Time, above: &mut Queue<jig::Event>, below: &mut Queue<io::Request>) {
        if let Some(command) = &mut self.command {
            command.command.fire(now, below);
        }
        self.answer(above);
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        self.command.as_ref()?.command.next_deadline()
    }
    fn answer(&mut self, above: &mut Queue<jig::Event>) {
        let Some(command) = &mut self.command else { return };
        let Some(answer) = command.command.answer() else { return };
        let answer = match answer {
            SourceAnswer::Check(check) => sources::Answer::Checked(match check {
                smith::ProgramCheck::Usable => jig::LoginState::Usable,
                smith::ProgramCheck::NotSignedIn => jig::LoginState::NotSignedIn,
                smith::ProgramCheck::SignedInOtherwise(method) => jig::LoginState::SignedInOtherwise(
                    jig::LoginMethod::new(method.as_bytes()).expect("same printable method grammar"),
                ),
                smith::ProgramCheck::Failed(failure) => {
                    jig::LoginState::Failed { reason: reason(failure.reason), detail: failure.detail }
                }
            }),
            SourceAnswer::SignOut(sign_out) => sources::Answer::SignedOut(match sign_out {
                smith::ProgramSignOut::SignedOut => jig::SignOutResult::SignedOut,
                smith::ProgramSignOut::Failed(failure) => {
                    jig::SignOutResult::Failed { reason: reason(failure.reason), detail: failure.detail }
                }
            }),
        };
        above.push(jig::Event::SourceAnswered { owner: command.owner, answer });
        self.command = None;
    }
}
fn reason(reason: smith::FailureReason) -> jig::SourceFailure {
    match reason {
        smith::FailureReason::Missing => jig::SourceFailure::Missing,
        smith::FailureReason::Deadline => jig::SourceFailure::Deadline,
        smith::FailureReason::Unreadable => jig::SourceFailure::Unreadable,
        smith::FailureReason::Exited => jig::SourceFailure::Exited,
    }
}
fn failed(
    owner: Token,
    operation: sources::Operation,
    reason: jig::SourceFailure,
    detail: &[u8],
    bound: u32,
) -> jig::Event {
    let detail = Box::from(
        detail.get(..detail.len().min(usize::try_from(bound).expect("detail bound"))).expect("bounded detail"),
    );
    let answer = match operation {
        sources::Operation::Check => sources::Answer::Checked(jig::LoginState::Failed { reason, detail }),
        sources::Operation::SignOut => sources::Answer::SignedOut(jig::SignOutResult::Failed { reason, detail }),
    };
    jig::Event::SourceAnswered { owner, answer }
}
