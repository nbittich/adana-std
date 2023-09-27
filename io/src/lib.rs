use adana_script_core::primitive::{Compiler, NativeFunctionCallResult, Primitive};

#[no_mangle]
pub fn read_line(params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    let message = if params.is_empty() {
        "".into()
    } else {
        params
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let stdin = std::io::stdin();

    print!("{message}");
    let mut buf = String::with_capacity(100);
    stdin.read_line(&mut buf)?;
    Ok(Primitive::String(buf))
}
