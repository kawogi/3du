#![warn(clippy::all, clippy::pedantic)]
// TODO re-enable this later and review all occurrences
#![allow(clippy::cast_precision_loss)]

use std::{fs::read_to_string, path::Path, sync::mpsc::Sender};

use log::{error, info};
use rustpython_vm::{
    compiler::Mode, pyclass, pymodule, PyObject, PyPayload, PyResult, TryFromBorrowedObject,
    VirtualMachine,
};

use crate::{api::Api, scene::Command};

#[allow(clippy::missing_panics_doc)]
pub fn runner(source_path: &(impl AsRef<Path> + ToString), sender: Sender<Command>, api: &Api) {
    let source = read_to_string(source_path).unwrap();
    let path_string = source_path.as_ref().display().to_string();

    rust_py_module::COMMAND_QUEUE
        .lock()
        .unwrap()
        .replace(sender);

    let interpreter = rustpython::InterpreterConfig::new()
        .init_stdlib()
        .init_hook(Box::new(|vm| {
            vm.add_native_module(
                "robot_api".to_owned(),
                Box::new(rust_py_module::make_module),
            );

            // vm.add_native_module(
            //     "robot_api2".to_owned(),
            //     Box::new(|vm: &VirtualMachine| {
            //         let module = PyModule::new();
            //         // ???
            //         module.into_ref(&vm.ctx)
            //     }),
            // );
        }))
        .interpreter();

    interpreter.enter(|vm| {
        vm.insert_sys_path(vm.new_pyobj("python"))
            .expect("add path");

        match vm.import("robot", 0) {
            Ok(module) => {
                let init_fn = module.get_attr("python_callback", vm).unwrap();
                init_fn.call((), vm).unwrap();

                let take_string_fn = module.get_attr("take_string", vm).unwrap();
                take_string_fn
                    .call((String::from("Rust string sent to python"),), vm)
                    .unwrap();
            }
            Err(exc) => {
                let mut msg = String::new();
                vm.write_exception(&mut msg, &exc).unwrap();
                panic!("{msg}");
            }
        }

        let scope = vm.new_scope_with_builtins();
        let compile = vm.compile(&source, Mode::Exec, path_string);

        match compile {
            Ok(py_code) => match vm.run_code_obj(py_code, scope) {
                Ok(code_result) => {
                    info!("Success: {code_result:?}");
                }
                Err(exception) => {
                    let mut output = String::new();
                    vm.write_exception(&mut output, &exception).unwrap();
                    error!("Syntax error: {output}");
                }
            },
            Err(err) => {
                let exception = vm.new_syntax_error(&err, Some(&source));
                let mut output = String::new();
                vm.write_exception(&mut output, &exception).unwrap();
                error!("Runtime error: {output}");
            }
        }
    });
}

#[pymodule]
// those are required by the Python API
#[allow(
    clippy::unnecessary_wraps,
    clippy::needless_pass_by_value,
    clippy::unused_self
)]
mod rust_py_module {
    use std::{
        sync::{atomic::Ordering, mpsc::Sender, Mutex},
        thread,
        time::Duration,
    };

    use crate::{api::Identifier, scene::Command, ROTATION};

    use super::{pyclass, PyObject, PyPayload, PyResult, TryFromBorrowedObject, VirtualMachine};
    use rustpython::vm::{builtins::PyList, convert::ToPyObject, PyObjectRef};

    pub(super) static COMMAND_QUEUE: Mutex<Option<Sender<Command>>> = Mutex::new(None);

    #[pyfunction]
    fn rust_function(
        num: i32,
        str: String,
        python_person: PythonPerson,
        _vm: &VirtualMachine,
    ) -> PyResult<RustStruct> {
        println!(
            "Calling standalone rust function from python passing args:
num: {},
string: {},
python_person.name: {}",
            num, str, python_person.name
        );
        Ok(RustStruct {
            numbers: NumVec(vec![1, 2, 3, 4]),
        })
    }

    #[pyfunction]
    fn move_forward() {
        COMMAND_QUEUE
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .send(Command {
                name: Identifier("MoveForward".to_owned()),
            })
            .unwrap();
        thread::sleep(Duration::from_millis(1000));
    }

    #[pyfunction]
    fn turn_left() {
        COMMAND_QUEUE
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .send(Command {
                name: Identifier("TurnLeft".to_owned()),
            })
            .unwrap();
        thread::sleep(Duration::from_millis(1000));
    }

    #[pyfunction]
    fn turn_right() {
        COMMAND_QUEUE
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .send(Command {
                name: Identifier("TurnRight".to_owned()),
            })
            .unwrap();
        thread::sleep(Duration::from_millis(1000));
    }

    #[pyfunction]
    fn rotate_cube(angle: u16) {
        println!("angle {angle}");
        ROTATION.store(angle, Ordering::Relaxed);
    }

    #[derive(Debug, Clone)]
    struct NumVec(Vec<i32>);

    impl ToPyObject for NumVec {
        fn to_pyobject(self, vm: &VirtualMachine) -> PyObjectRef {
            let list = self.0.into_iter().map(|item| vm.new_pyobj(item)).collect();
            PyList::new_ref(list, vm.as_ref()).to_pyobject(vm)
        }
    }

    #[pyattr]
    #[pyclass(module = "rust_py_module", name = "RustStruct")]
    #[derive(Debug, PyPayload)]
    struct RustStruct {
        numbers: NumVec,
    }

    #[pyclass]
    impl RustStruct {
        #[pygetset]
        fn numbers(&self) -> NumVec {
            self.numbers.clone()
        }

        #[pymethod]
        fn print_in_rust_from_python(&self) {
            println!("Calling a rust method from python");
        }
    }

    struct PythonPerson {
        name: String,
    }

    impl<'obj> TryFromBorrowedObject<'obj> for PythonPerson {
        fn try_from_borrowed_object(vm: &VirtualMachine, obj: &'obj PyObject) -> PyResult<Self> {
            let name = obj.get_attr("name", vm)?.try_into_value::<String>(vm)?;
            Ok(PythonPerson { name })
        }
    }
}
