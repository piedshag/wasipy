use python::python_exec;

mod python;

wit_bindgen::generate!({
    world: "executor-world",
});

struct Executor;

impl Guest for Executor {
    fn run(script: String) -> Result<String, String> {
        python_exec(&script).map_err(|e| e.to_string())
    }
}

export!(Executor);
