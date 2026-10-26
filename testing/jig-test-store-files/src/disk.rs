//! Full directory opens for worlds that exercise patch sets. Ordinary worlds
//! retain the cheaper memory maps and direct positioned-file answers.
use jig_protocol_store::{Arrived, Limits, Root, Seeded};
use skein_fake_machine::{How, Machine, serve};
use skein_io::{
    file,
    file_layer::{self, FileIo},
};
use skein_lib::{Queue, Time, Token};
use skein_sim::{Config, Handle, Pid, Sim};

// The store world's file-driver queue capacity; each request settles before
// the next, with room for all completions of the eight driver flights.
const QUEUE: u32 = 128;

pub(super) struct Disk {
    machine: Machine,
    sim: Sim,
    pid: Pid,
    root: Root,
    io: [FileIo; 3],
    seeded: Seeded,
    scratch: Option<Root>,
}
impl Disk {
    pub fn new(limits: &Limits) -> Self {
        let mut machine = Machine::for_user(0);
        let handle = machine.lay(&[]);
        let scratch = machine.lay(&[]);
        machine.close(scratch);
        let (sim, pid, root, io, seeded) = drivers(limits, handle, 0);
        Self { machine, sim, pid, root, io, seeded, scratch: None }
    }
    pub fn root(&self) -> Root {
        self.root
    }
    pub fn scratch(&mut self) -> Root {
        if let Some(root) = self.scratch {
            return root;
        }
        let handle = self.machine.reopen_root(1);
        let fd = self.sim.root(self.pid, Handle::new(handle.raw()));
        let tokens: Vec<_> = self.io.iter_mut().map(|io| io.adopt_root(fd).expect("scratch root slot")).collect();
        assert!(tokens.iter().all(|token| *token == tokens[0]), "roots adopted in the same order");
        let root = Root { token: tokens[0], fd };
        self.scratch = Some(root);
        root
    }
    pub fn seeded(&self) -> Seeded {
        self.seeded
    }
    pub fn crash(&mut self, limits: &Limits, seed: u64) {
        self.scratch = None;
        self.machine.crash(seed);
        let handle = self.machine.reopen_root(0);
        (self.sim, self.pid, self.root, self.io, self.seeded) = drivers(limits, handle, seed);
    }
    pub fn unread_transcript(&mut self, task: u64) {
        let root = self.machine.reopen_root(0);
        let name = format!("transcripts/{:016x}/{task:016x}", task >> 12);
        let file = self.machine.open(root, name.as_bytes(), How::Write).expect("kept transcript");
        self.machine.truncate(file, 0).expect("writable transcript");
        self.machine.sync(file);
        self.machine.close(file);
        self.machine.close(root);
    }
    /// Stable file bytes beneath one directory, for generation-isolation stories.
    pub fn contents(&mut self, name: &[u8]) -> std::collections::BTreeMap<Vec<u8>, Vec<u8>> {
        let root = self.machine.reopen_root(0);
        let mut result = std::collections::BTreeMap::new();
        collect(&mut self.machine, root, name, &mut result);
        self.machine.close(root);
        result
    }
    pub fn read(&mut self, name: &[u8]) -> Option<Vec<u8>> {
        let root = self.machine.reopen_root(0);
        let bytes = self.machine.open(root, name, How::ReadNoFollow).ok().map(|file| {
            let bytes = self.machine.read(file, 0, u32::MAX).expect("scenario file");
            self.machine.close(file);
            bytes
        });
        self.machine.close(root);
        bytes
    }
    pub fn answer(&mut self, driver: usize, request: file::Request) -> Arrived {
        let io = &mut self.io[driver];
        let mut events = Queue::with_capacity(QUEUE);
        let mut submissions = Queue::with_capacity(QUEUE);
        let mut completions = Queue::with_capacity(QUEUE);
        file_layer::down(io, Time::ZERO, request, &mut events, &mut submissions);
        // The calm simulator answers one file operation synchronously; the
        // bound also guards a broken file driver in this ordinary test shell.
        for _ in 0..100 {
            if !submissions.is_empty() {
                self.sim.submit(self.pid, &mut submissions);
                serve(&mut self.machine, &mut self.sim);
            }
            self.sim.reap(self.pid, &mut completions);
            while let Some(complete) = completions.pop() {
                file_layer::up(io, complete, &mut events, &mut submissions);
            }
            if io.takes() {
                break;
            }
        }
        assert!(io.takes(), "calm file operation settles");
        let event = events.pop().expect("one file terminal");
        assert!(events.is_empty(), "one terminal per request");
        let fd = if let file::Event::Opened { file, .. } = &event { io.descriptor(*file) } else { None };
        Arrived { event, fd }
    }
}
fn drivers(limits: &Limits, handle: skein_fake_machine::Opened, seed: u64) -> (Sim, Pid, Root, [FileIo; 3], Seeded) {
    let mut sim = Sim::new(seed, Config::calm());
    let pid = sim.spawn_process();
    let fd = sim.root(pid, Handle::new(handle.raw()));
    let mut io = std::array::from_fn(|_| {
        let whole = limits.store.chunk.max(jig_protocol_store::sizes::MANIFEST_BYTES);
        let mut driver =
            FileIo::with_whole_limit(8, limits.store.chunk, 32, whole, limits.store.deadline).with_effective_user(0);
        assert_eq!(driver.adopt_root(fd), Some(Token::new(1)));
        driver
    });
    let [store, secrets, transcripts] = &mut io;
    let seeded = jig_protocol_store::seed([store, secrets, transcripts], seed);
    (sim, pid, Root { token: Token::new(1), fd }, io, seeded)
}

fn collect(
    machine: &mut Machine,
    root: skein_fake_machine::Opened,
    name: &[u8],
    into: &mut std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
) {
    let file = machine.open(root, name, How::ReadNoFollow).expect("generation path");
    if machine.stat(file).is == skein_fake_machine::Is::Directory {
        let entries = machine.list(file, 32, 8192).expect("bounded fixture directory");
        assert!(entries.len() < 32, "fixture fits one directory page");
        for (_, entry_name) in entries {
            let mut child = name.to_vec();
            child.push(b'/');
            child.extend_from_slice(&entry_name);
            collect(machine, root, &child, into);
        }
    } else {
        into.insert(name.to_vec(), machine.read(file, 0, u32::MAX).expect("generation bytes"));
    }
    machine.close(file);
}
