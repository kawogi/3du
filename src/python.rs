#![warn(clippy::all, clippy::pedantic)]
// TODO re-enable this later and review all occurrences
#![allow(clippy::cast_precision_loss)]

// TODO enable hand-picked clippy lints from the `restriction` group

use std::{fs::read_to_string, path::Path};

use log::{error, info};
use rustpython_vm::{
    compiler::Mode, pyclass, pymodule, PyObject, PyPayload, PyResult, TryFromBorrowedObject,
    VirtualMachine,
};

pub(crate) fn python_runner(source_path: &(impl AsRef<Path> + ToString)) {
    let source = read_to_string(source_path).unwrap();
    let path_string = source_path.as_ref().display().to_string();

    let interpreter = rustpython::InterpreterConfig::new()
        .init_stdlib()
        .init_hook(Box::new(|vm| {
            vm.add_native_module(
                "rust_py_module".to_owned(),
                Box::new(rust_py_module::make_module),
            );
        }))
        .interpreter();

    interpreter.enter(|vm| {
        vm.insert_sys_path(vm.new_pyobj("python"))
            .expect("add path");

        match vm.import("call_between_rust_and_python", 0) {
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
    use super::{pyclass, PyObject, PyPayload, PyResult, TryFromBorrowedObject, VirtualMachine};
    use rustpython::vm::{builtins::PyList, convert::ToPyObject, PyObjectRef};

    #[pyfunction]
    fn rust_function(
        num: i32,
        s: String,
        python_person: PythonPerson,
        _vm: &VirtualMachine,
    ) -> PyResult<RustStruct> {
        println!(
            "Calling standalone rust function from python passing args:
num: {},
string: {},
python_person.name: {}",
            num, s, python_person.name
        );
        Ok(RustStruct {
            numbers: NumVec(vec![1, 2, 3, 4]),
        })
    }

    #[derive(Debug, Clone)]
    struct NumVec(Vec<i32>);

    impl ToPyObject for NumVec {
        fn to_pyobject(self, vm: &VirtualMachine) -> PyObjectRef {
            let list = self.0.into_iter().map(|e| vm.new_pyobj(e)).collect();
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

    impl<'a> TryFromBorrowedObject<'a> for PythonPerson {
        fn try_from_borrowed_object(vm: &VirtualMachine, obj: &'a PyObject) -> PyResult<Self> {
            let name = obj.get_attr("name", vm)?.try_into_value::<String>(vm)?;
            Ok(PythonPerson { name })
        }
    }
}