//! Runs one ADG-2 execution candidate against the shared mod body.
//!
//! The four candidates bind the same mod four ways: a native dynamic library, a
//! WebAssembly module, a helper process, and a Rust library linked straight in.
//! Everything above the binding — workload, warm-up, batching, failure matrix,
//! and report shape — is written once, so what the numbers differ by is the
//! binding.
//!
//! Measurement is generic over the binding rather than dynamically dispatched:
//! a `dyn` call in the hot loop would cost more than some of the bindings do.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{BufReader, Read as _, Write as _, stdin, stdout};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use libloading::{Library, Symbol};
use ravel_spike_adg_2_mod_execution as protocol;
use ravel_spike_adg2_guest as guest;
use ravel_spike_harness::{READY_MARKER, ReportedFailure, TrialReport, linux_rss};
use wasmtime::{Engine, Instance, Linker, Module, Store, TypedFunc};

/// Bytes a single call moves across the boundary: one argument, one result.
const BOUNDARY_BYTES_PER_CALL: u64 = 16;

#[global_allocator]
static ALLOCATOR: Counting = Counting;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

/// Counts what the host allocates, so per-call allocation is measured rather
/// than assumed.
struct Counting;

// Safety: every method forwards to the system allocator unchanged and only adds
// two relaxed counters.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

fn main() -> ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("host failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<(), String> {
    let candidate = option(arguments, "--candidate").unwrap_or_else(|| "static".to_owned());
    let mode = option(arguments, "--mode").unwrap_or_else(|| "measure".to_owned());
    let plan = Plan {
        warmup: number(arguments, "--warmup", 200_000)?,
        samples: number(arguments, "--samples", 20_000)?,
        batch: number(arguments, "--batch", 64)?,
    };
    let artifact = option(arguments, "--artifact");

    match candidate.as_str() {
        "dylib" => drive(
            &mut DylibMod::load(&require(artifact, "dylib")?)?,
            &candidate,
            &mode,
            plan,
        ),
        "wasm" => drive(
            &mut WasmMod::load(&require(artifact, "wasm")?)?,
            &candidate,
            &mode,
            plan,
        ),
        "process" => drive(
            &mut ProcessMod::spawn(&require(artifact, "process")?)?,
            &candidate,
            &mode,
            plan,
        ),
        "static" => drive(&mut StaticMod::link(), &candidate, &mode, plan),
        unknown => Err(format!("unknown candidate {unknown}")),
    }
}

/// How much work one trial does.
#[derive(Clone, Copy)]
struct Plan {
    warmup: u64,
    samples: u64,
    batch: u64,
}

/// What refusing an absent entry point looked like.
enum Refusal {
    /// The host learned the entry was absent and said so.
    Reported(String),
    /// The mod answered anyway, which is the failure this probe looks for.
    Accepted,
}

/// One way of binding the shared mod body to this host.
trait ModHost {
    /// Runs the mod body.
    fn call(&mut self, payload: u64) -> Result<u64, String>;
    /// Runs a mod-to-mod call, which the mod routes back through this host.
    fn peer(&mut self, payload: u64) -> Result<u64, String>;
    /// Reports the contract version the mod was built against.
    fn abi_version(&mut self) -> Result<u32, String>;
    /// Asks for an entry point the mod does not have.
    fn missing_entry(&mut self) -> Result<Refusal, String>;
    /// Ends the mod process-fatally, if this binding lets that reach the host.
    fn crash(&mut self) -> Result<(), String>;
    /// Resident set size of any process this binding owns.
    fn auxiliary_rss(&self) -> Option<u64> {
        None
    }
}

/// The host function every binding offers the mod for mod-to-mod calls.
///
/// A second instance of the same body stands in for the second mod: what is
/// being measured is the cost of the hop through the host, not the peer's work.
extern "C" fn host_peer(payload: u64) -> u64 {
    guest::call(payload)
}

fn drive<M: ModHost>(host: &mut M, candidate: &str, mode: &str, plan: Plan) -> Result<(), String> {
    let started = Instant::now();
    match mode {
        "crash" => {
            host.crash()?;
            // Reaching this line is the observation: the binding contained it.
            println!("survived the mod ending itself");
            return Ok(());
        }
        "fail" => {
            // A mod body that panics either reaches the host as an error or ends
            // the process. Which one it is cannot be reported from inside the
            // trial that dies, so it is probed in its own process.
            let detail = match host.call(guest::FAILING_PAYLOAD) {
                Ok(value) => format!("the mod returned {value} after failing"),
                Err(detail) => detail,
            };
            println!("survived {detail}");
            return Ok(());
        }
        "hang" => {
            // Never reports, so the harness timeout is what ends this trial.
            loop {
                std::thread::park();
            }
        }
        "abi" => {
            let version = host.abi_version()?;
            match host.missing_entry()? {
                Refusal::Reported(detail) => println!("refused {version} {detail}"),
                Refusal::Accepted => println!("accepted {version}"),
            }
            return Ok(());
        }
        "measure" => {}
        unknown => return Err(format!("unknown mode {unknown}")),
    }

    // The harness samples memory at this marker, before any measurement.
    let ready_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    println!("{READY_MARKER}");
    stdout().flush().map_err(|error| error.to_string())?;

    let mut batches = vec![0_u64; usize::try_from(plan.samples).unwrap_or(0)];
    time(host, plan, &mut batches, ModHost::call)?;

    let allocations_before = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes_before = ALLOCATED_BYTES.load(Ordering::Relaxed);
    time(
        host,
        Plan { warmup: 0, ..plan },
        &mut batches,
        ModHost::call,
    )?;
    let invocations = plan.samples * plan.batch;
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - allocations_before;
    let allocated_bytes = ALLOCATED_BYTES.load(Ordering::Relaxed) - bytes_before;

    // A peer call is two boundary crossings, so it gets its own, shorter window.
    let peer_plan = Plan {
        warmup: plan.warmup / 10,
        samples: plan.samples / 10,
        batch: plan.batch,
    };
    let mut peer_batches = vec![0_u64; usize::try_from(peer_plan.samples).unwrap_or(0)];
    time(host, peer_plan, &mut peer_batches, ModHost::peer)?;

    emit(&TrialReport {
        candidate: candidate.to_owned(),
        ready_ms,
        warmup: plan.warmup,
        batch_nanos: batches,
        peer_batch_nanos: peer_batches,
        boundary_bytes_per_call: BOUNDARY_BYTES_PER_CALL,
        allocations_per_call: Some(rate(allocations, invocations)),
        allocated_bytes_per_call: Some(rate(allocated_bytes, invocations)),
        auxiliary_rss_bytes: host.auxiliary_rss(),
        failures: failure_matrix(host)?,
        // A Rust host has no JVM, no classes, and nothing to transform.
        ..TrialReport::default()
    })?;

    // Park until the harness has read this process's memory.
    let mut release = [0_u8; 1];
    if stdin().read(&mut release).map_err(|e| e.to_string())? == 0 {
        return Err("the harness closed standard input without releasing".to_owned());
    }
    Ok(())
}

/// Times batches of one operation, reporting batch totals undivided.
///
/// The operation is a generic parameter rather than a function pointer, so the
/// binding under test is inlined into the loop instead of called through one.
fn time<M: ModHost, Op: Fn(&mut M, u64) -> Result<u64, String>>(
    host: &mut M,
    plan: Plan,
    batches: &mut [u64],
    operation: Op,
) -> Result<(), String> {
    let mut sink = 0_u64;
    for index in 0..plan.warmup {
        sink ^= operation(host, payload(index))?;
    }
    for slot in batches.iter_mut() {
        let started = Instant::now();
        for index in 0..plan.batch {
            sink ^= operation(host, payload(index))?;
        }
        *slot = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    }
    // Keep the accumulator observable so the loop above cannot be elided.
    if sink == u64::MAX {
        eprintln!("{sink}");
    }
    Ok(())
}

/// Payloads stay clear of the sentinels the failure matrix uses.
fn payload(index: u64) -> u64 {
    guest::FIRST_MEASURED_PAYLOAD + index
}

#[expect(
    clippy::cast_precision_loss,
    reason = "measured counts stay far below f64 integer precision limits"
)]
fn rate(total: u64, invocations: u64) -> f64 {
    if invocations == 0 {
        return 0.0;
    }
    total as f64 / invocations as f64
}

/// Exercises the scenarios a host can observe without dying of them.
///
/// A mod body that fails is probed separately, because some bindings cannot
/// survive it and a dead host reports nothing.
fn failure_matrix<M: ModHost>(host: &mut M) -> Result<Vec<ReportedFailure>, String> {
    let mut failures = Vec::new();

    let invalid = host.call(guest::INVALID_PAYLOAD);
    failures.push(outcome(
        "invalid-input",
        match invalid {
            Ok(guest::REJECTED) => (true, "the mod returned its rejection value".to_owned()),
            Ok(_) => (false, "the mod accepted invalid input".to_owned()),
            Err(detail) => (true, detail),
        },
    ));

    failures.push(outcome(
        "unavailable-runtime",
        match host.missing_entry()? {
            Refusal::Reported(detail) => (true, detail),
            Refusal::Accepted => (
                false,
                "the mod answered for an entry point it does not have".to_owned(),
            ),
        },
    ));
    Ok(failures)
}

fn outcome(scenario: &str, observed: (bool, String)) -> ReportedFailure {
    let (closed, detail) = observed;
    ReportedFailure {
        scenario: scenario.to_owned(),
        outcome: if closed { "failed-closed" } else { "continued" }.to_owned(),
        detail,
    }
}

fn emit(report: &TrialReport) -> Result<(), String> {
    let line = serde_json::to_string(report)
        .map_err(|error| format!("cannot serialize the report: {error}"))?;
    println!("{line}");
    stdout().flush().map_err(|error| error.to_string())
}

/// The mod as a native dynamic library, resolved by symbol name.
///
/// Symbols are resolved once at load, because that is what a loader does. A
/// lookup per call would measure the dynamic linker's hash table instead.
struct DylibMod {
    call: extern "C" fn(u64) -> u64,
    peer: extern "C" fn(u64) -> u64,
    abi_version: extern "C" fn() -> u32,
    crash: extern "C" fn(),
    // Declared last so it outlives the pointers taken out of it.
    library: Library,
}

impl DylibMod {
    fn load(path: &str) -> Result<Self, String> {
        // Safety: the path names the guest cdylib this experiment built, whose
        // initializers are the ones in this repository.
        let library = unsafe { Library::new(path) }
            .map_err(|error| format!("cannot load the mod library: {error}"))?;

        let bind = *symbol::<extern "C" fn(guest::Peer)>(&library, b"ravel_mod_bind_peer")?;
        bind(host_peer);
        Ok(Self {
            call: *symbol(&library, b"ravel_mod_call")?,
            peer: *symbol(&library, b"ravel_mod_peer")?,
            abi_version: *symbol(&library, b"ravel_mod_abi_version")?,
            crash: *symbol(&library, b"ravel_mod_crash")?,
            library,
        })
    }
}

fn symbol<'library, T>(
    library: &'library Library,
    name: &[u8],
) -> Result<Symbol<'library, T>, String> {
    // Safety: each name is a symbol this experiment exported with the signature
    // the call site declares.
    unsafe { library.get(name) }.map_err(|error| {
        format!(
            "the mod has no {} entry point: {error}",
            String::from_utf8_lossy(name)
        )
    })
}

impl ModHost for DylibMod {
    fn call(&mut self, payload: u64) -> Result<u64, String> {
        Ok((self.call)(payload))
    }

    fn peer(&mut self, payload: u64) -> Result<u64, String> {
        Ok((self.peer)(payload))
    }

    fn abi_version(&mut self) -> Result<u32, String> {
        Ok((self.abi_version)())
    }

    fn missing_entry(&mut self) -> Result<Refusal, String> {
        Ok(
            match symbol::<extern "C" fn()>(&self.library, b"ravel_mod_absent") {
                Ok(_) => Refusal::Accepted,
                Err(_) => Refusal::Reported(
                    "the dynamic loader reported no such symbol before any call".to_owned(),
                ),
            },
        )
    }

    fn crash(&mut self) -> Result<(), String> {
        (self.crash)();
        Ok(())
    }
}

/// The mod as a WebAssembly module in a sandbox this host owns.
struct WasmMod {
    store: Store<()>,
    call: TypedFunc<u64, u64>,
    peer: TypedFunc<u64, u64>,
    abi_version: TypedFunc<(), u32>,
    crash: TypedFunc<(), ()>,
    instance: Instance,
}

impl WasmMod {
    fn load(path: &str) -> Result<Self, String> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, path)
            .map_err(|error| format!("cannot compile the mod module: {error}"))?;
        let mut linker = Linker::new(&engine);
        linker
            .func_wrap(
                "ravel",
                "ravel_host_peer",
                |_caller: wasmtime::Caller<'_, ()>, payload: u64| host_peer(payload),
            )
            .map_err(|error| format!("cannot provide the host function: {error}"))?;

        let mut store = Store::new(&engine, ());
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|error| format!("cannot instantiate the mod: {error}"))?;

        let typed = |store: &mut Store<()>, name: &str| {
            instance
                .get_func(&mut *store, name)
                .ok_or_else(|| format!("the mod has no {name} export"))
        };
        let call = typed(&mut store, "ravel_mod_call")?
            .typed(&store)
            .map_err(|error| format!("ravel_mod_call has the wrong shape: {error}"))?;
        let peer = typed(&mut store, "ravel_mod_peer")?
            .typed(&store)
            .map_err(|error| format!("ravel_mod_peer has the wrong shape: {error}"))?;
        let abi_version = typed(&mut store, "ravel_mod_abi_version")?
            .typed(&store)
            .map_err(|error| format!("ravel_mod_abi_version has the wrong shape: {error}"))?;
        let crash = typed(&mut store, "ravel_mod_crash")?
            .typed(&store)
            .map_err(|error| format!("ravel_mod_crash has the wrong shape: {error}"))?;

        Ok(Self {
            store,
            call,
            peer,
            abi_version,
            crash,
            instance,
        })
    }
}

impl ModHost for WasmMod {
    fn call(&mut self, payload: u64) -> Result<u64, String> {
        self.call
            .call(&mut self.store, payload)
            .map_err(|error| format!("the sandbox trapped: {}", first_line(&error.to_string())))
    }

    fn peer(&mut self, payload: u64) -> Result<u64, String> {
        self.peer
            .call(&mut self.store, payload)
            .map_err(|error| format!("the sandbox trapped: {}", first_line(&error.to_string())))
    }

    fn abi_version(&mut self) -> Result<u32, String> {
        self.abi_version
            .call(&mut self.store, ())
            .map_err(|error| format!("the sandbox trapped: {}", first_line(&error.to_string())))
    }

    fn missing_entry(&mut self) -> Result<Refusal, String> {
        Ok(
            match self.instance.get_func(&mut self.store, "ravel_mod_absent") {
                Some(_) => Refusal::Accepted,
                None => Refusal::Reported(
                    "the module declares no such export, refused before any call".to_owned(),
                ),
            },
        )
    }

    fn crash(&mut self) -> Result<(), String> {
        match self.crash.call(&mut self.store, ()) {
            Ok(()) => Ok(()),
            // A trap is the sandbox containing the crash, which is the point.
            Err(error) => {
                eprintln!("the sandbox trapped: {}", first_line(&error.to_string()));
                Ok(())
            }
        }
    }
}

/// The mod in a helper process, reached one frame at a time.
struct ProcessMod {
    worker: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl ProcessMod {
    fn spawn(path: &str) -> Result<Self, String> {
        let mut worker = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| format!("cannot start the mod process: {error}"))?;
        let input = worker
            .stdin
            .take()
            .ok_or_else(|| "the mod process has no standard input".to_owned())?;
        let output = worker
            .stdout
            .take()
            .ok_or_else(|| "the mod process has no standard output".to_owned())?;
        Ok(Self {
            worker,
            input,
            output: BufReader::new(output),
        })
    }

    /// Sends one request and answers any peer request the mod makes on the way.
    fn request(&mut self, opcode: u8, payload: u64) -> Result<(u8, u64), String> {
        protocol::write_frame(&mut self.input, opcode, payload)
            .map_err(|error| format!("cannot reach the mod process: {error}"))?;
        loop {
            let frame = protocol::read_frame(&mut self.output)
                .map_err(|error| format!("cannot read from the mod process: {error}"))?;
            let Some((tag, value)) = frame else {
                return Err("the mod process ended without answering".to_owned());
            };
            if tag == protocol::TAG_PEER_REQUEST {
                let answer = host_peer(value);
                protocol::write_frame(&mut self.input, protocol::TAG_RESULT, answer)
                    .map_err(|error| format!("cannot answer the mod process: {error}"))?;
                continue;
            }
            return Ok((tag, value));
        }
    }
}

impl ModHost for ProcessMod {
    fn call(&mut self, payload: u64) -> Result<u64, String> {
        match self.request(protocol::OPCODE_CALL, payload)? {
            (protocol::TAG_RESULT, value) => Ok(value),
            (tag, _) => Err(format!("the mod process refused with tag {tag}")),
        }
    }

    fn peer(&mut self, payload: u64) -> Result<u64, String> {
        match self.request(protocol::OPCODE_PEER, payload)? {
            (protocol::TAG_RESULT, value) => Ok(value),
            (tag, _) => Err(format!("the mod process refused with tag {tag}")),
        }
    }

    fn abi_version(&mut self) -> Result<u32, String> {
        let (_, value) = self.request(protocol::OPCODE_ABI_VERSION, 0)?;
        u32::try_from(value).map_err(|_| "the mod reported an impossible version".to_owned())
    }

    fn missing_entry(&mut self) -> Result<Refusal, String> {
        Ok(match self.request(protocol::OPCODE_UNKNOWN_ENTRY, 0)? {
            (protocol::TAG_REFUSED, _) => {
                Refusal::Reported("the mod process refused an unknown entry point".to_owned())
            }
            _ => Refusal::Accepted,
        })
    }

    fn crash(&mut self) -> Result<(), String> {
        // The request cannot be answered, so a broken pipe is the expected path.
        drop(protocol::write_frame(
            &mut self.input,
            protocol::OPCODE_CRASH,
            0,
        ));
        let status = self
            .worker
            .wait()
            .map_err(|error| format!("cannot wait for the mod process: {error}"))?;
        eprintln!("the mod process ended with {status}");
        Ok(())
    }

    fn auxiliary_rss(&self) -> Option<u64> {
        linux_rss(Path::new("/proc"), self.worker.id())
            .ok()
            .map(|snapshot| snapshot.current_bytes)
    }
}

/// The mod linked into the host, called directly.
struct StaticMod;

impl StaticMod {
    fn link() -> Self {
        guest::ravel_mod_bind_peer(host_peer);
        Self
    }
}

impl ModHost for StaticMod {
    fn call(&mut self, payload: u64) -> Result<u64, String> {
        // A linked mod unwinds into its host, so the host can catch it. This
        // guard is what production would need, so it is measured, not excluded.
        std::panic::catch_unwind(|| guest::call(payload))
            .map_err(|_| "the mod body panicked and the host caught it".to_owned())
    }

    fn peer(&mut self, payload: u64) -> Result<u64, String> {
        Ok(guest::peer(payload))
    }

    fn abi_version(&mut self) -> Result<u32, String> {
        Ok(guest::ABI_VERSION)
    }

    fn missing_entry(&mut self) -> Result<Refusal, String> {
        // A linked mod cannot be missing an entry point: the build would have
        // failed. There is nothing to observe at run time.
        Ok(Refusal::Reported(
            "a linked mod cannot lack an entry point; the link would have failed".to_owned(),
        ))
    }

    fn crash(&mut self) -> Result<(), String> {
        guest::ravel_mod_crash();
        Ok(())
    }
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_owned()
}

fn option(arguments: &[String], name: &str) -> Option<String> {
    arguments
        .iter()
        .position(|argument| argument == name)
        .and_then(|index| arguments.get(index + 1))
        .cloned()
}

fn number(arguments: &[String], name: &str, fallback: u64) -> Result<u64, String> {
    match option(arguments, name) {
        Some(value) => value
            .parse()
            .map_err(|_| format!("{name} needs a whole number")),
        None => Ok(fallback),
    }
}

fn require(value: Option<String>, candidate: &str) -> Result<String, String> {
    value.ok_or_else(|| format!("the {candidate} candidate needs --artifact"))
}
