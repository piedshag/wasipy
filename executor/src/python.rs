use std::cell::RefCell;
use std::sync::Arc;

use anyhow::Result;
use rustpython::InterpreterConfig;
use rustpython::vm::builtins::PyStrRef;
use rustpython::vm::{
    self, PyObjectRef, PyRef, PyResult, VirtualMachine, builtins::PyBaseExceptionRef, extend_class,
    py_class,
};

pub fn python_exec(code: &str) -> Result<String> {
    let stdout_buffer = Arc::new(RefCell::new(Vec::new()));

    let stdout_clone = Arc::clone(&stdout_buffer);
    let write_fn = move |data: &str, _vm: &VirtualMachine| -> PyResult<()> {
        match stdout_clone.try_borrow_mut() {
            Ok(mut stdout) => {
                stdout.extend_from_slice(data.as_bytes());
                Ok(())
            }
            Err(_) => {
                panic!("Could not borrow stdout buffer mutably");
            }
        }
    };

    InterpreterConfig::new()
        .init_stdlib()
        .interpreter()
        .enter(|vm| {
            let scope = vm.new_scope_with_builtins();

            vm.sys_module
                .set_attr("stdout", make_stdout_object(vm, write_fn), vm)
                .map_err(|e| get_error(vm, e))?;

            match vm
                .compile(code, vm::compiler::Mode::Exec, "<embedded>".to_owned())
                .map_err(|err| vm.new_syntax_error(&err, Some(code)))
                .and_then(|code_obj| vm.run_code_obj(code_obj, scope.clone()))
            {
                Ok(output) => match output.str(vm) {
                    Ok(s) => Ok(s.to_string()),
                    Err(e) => Err(get_error(vm, e)),
                },
                Err(exc) => Err(get_error(vm, exc)),
            }
        })?;

    Ok(String::from_utf8(stdout_buffer.borrow().to_vec()).unwrap())
}

fn get_error(vm: &VirtualMachine, e: PyBaseExceptionRef) -> anyhow::Error {
    let mut s = String::new();
    let _ = vm.write_exception(&mut s, &e);
    anyhow::anyhow!(s)
}

pub fn make_stdout_object(
    vm: &VirtualMachine,
    write_f: impl Fn(&str, &VirtualMachine) -> PyResult<()> + 'static,
) -> PyObjectRef {
    let ctx = &vm.ctx;
    let cls = PyRef::leak(py_class!(
        ctx,
        "wasi_stdout",
        vm.ctx.types.object_type.to_owned(),
        {}
    ));
    let write_method = vm.new_method(
        "write",
        cls,
        move |_self: PyObjectRef, data: PyStrRef, vm: &VirtualMachine| -> PyResult<()> {
            write_f(data.as_str(), vm)
        },
    );
    let flush_method = vm.new_method("flush", cls, |_self: PyObjectRef| {});
    extend_class!(ctx, cls, {
        "write" => write_method,
        "flush" => flush_method,
    });
    ctx.new_base_object(cls.to_owned(), None)
}
