//! ADG-1 candidate that lets Rust own process start and host the JVM.
//!
//! Prism can run this binary as an instance wrapper command. It creates the VM
//! through the Invocation API, binds the hook callback with `RegisterNatives`
//! instead of an exported symbol, and then runs the same trial class the other
//! candidates run.
//!
//! Usage: `--classpath <path> [--agent <jar=args>] -- <trial arguments>`

use std::ffi::c_void;
use std::process::ExitCode;

use jni::errors::Error;
use jni::objects::{JObject, JObjectArray, JString, JValue};
use jni::{Env, InitArgsBuilder, JNIVersion, JavaVM, NativeMethod, jni_sig, jni_str};

use ravel_spike_adg1::callback;

fn main() -> ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("hosted trial failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<(), String> {
    let classpath = option(arguments, "--classpath")
        .ok_or_else(|| "the hosted candidate needs --classpath".to_owned())?;
    let agent = option(arguments, "--agent");
    let separator = arguments
        .iter()
        .position(|argument| argument == "--")
        .ok_or_else(|| "the hosted candidate needs `--` before trial arguments".to_owned())?;
    let trial_arguments = &arguments[separator + 1..];

    let mut builder = InitArgsBuilder::new()
        .version(JNIVersion::V1_8)
        .option(format!("-Djava.class.path={classpath}"))
        .option("--enable-native-access=ALL-UNNAMED");
    if let Some(agent) = agent {
        builder = builder.option(format!("-javaagent:{agent}"));
    }
    let init_arguments = builder
        .build()
        .map_err(|error| format!("cannot build JVM arguments: {error}"))?;

    let vm =
        JavaVM::new(init_arguments).map_err(|error| format!("cannot create a JVM: {error}"))?;
    vm.attach_current_thread(|env| register_and_run(env, trial_arguments))
        .map_err(|error: Error| format!("the hosted trial failed: {error}"))
}

fn register_and_run(env: &mut Env<'_>, trial_arguments: &[String]) -> Result<(), Error> {
    // Safety: `callback` has the static-native signature `(J)J` declares.
    let methods = [unsafe {
        NativeMethod::from_raw_parts(
            jni_str!("onNativeCallback"),
            jni_str!("(J)J"),
            callback as *mut c_void,
        )
    }];
    // Safety: the descriptor above matches the function it points at.
    unsafe { env.register_native_methods(jni_str!("dev/ravel/spike/Hook"), &methods) }?;

    let array = string_array(env, trial_arguments)?;
    env.call_static_method(
        jni_str!("dev/ravel/spike/Trial"),
        jni_str!("main"),
        jni_sig!("([Ljava/lang/String;)V"),
        &[JValue::Object(&array)],
    )?;
    Ok(())
}

fn string_array<'local>(
    env: &mut Env<'local>,
    values: &[String],
) -> Result<JObjectArray<'local>, Error> {
    let length = i32::try_from(values.len()).unwrap_or(i32::MAX);
    let array = env.new_object_array(length, jni_str!("java/lang/String"), JObject::null())?;
    for (index, value) in values.iter().enumerate() {
        let element: JString<'_> = env.new_string(value)?;
        array.set_element(env, index, &element)?;
    }
    Ok(array)
}

fn option(arguments: &[String], name: &str) -> Option<String> {
    arguments
        .iter()
        .position(|argument| argument == name)
        .and_then(|index| arguments.get(index + 1))
        .cloned()
}
