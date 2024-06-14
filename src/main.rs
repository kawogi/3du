#![warn(clippy::all, clippy::pedantic)]
// TODO re-enable this later and review all occurrences
#![allow(clippy::cast_precision_loss)]

// TODO enable hand-picked clippy lints from the `restriction` group

mod framework;
mod logging;
mod scene;

use std::{fs::read_to_string, path::Path, thread};

use logging::init_logger;
use rustpython_vm::{self as vm, Settings};

fn python_runner(source_path: &(impl AsRef<Path> + ToString)) {
    let source = read_to_string(source_path).unwrap();
    let py_result: vm::PyResult<()> =
        vm::Interpreter::without_stdlib(Settings::default()).enter(|vm| {
            let scope = vm.new_scope_with_builtins();
            let code_obj = vm
                .compile(&source, vm::compiler::Mode::Exec, source_path.to_string())
                .map_err(|err| vm.new_syntax_error(&err, Some(&source)))?;

            vm.run_code_obj(code_obj, scope)?;

            Ok(())
        });

    py_result.unwrap();
}

fn main() {
    init_logger();

    let source_path = "src/test.py";
    let python_tread = thread::spawn(move || python_runner(&source_path));

    pollster::block_on(framework::start("my scene"));

    python_tread.join().unwrap();
}
