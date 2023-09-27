use std::{collections::BTreeMap, io::Write};

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
    print!("{message}");
    std::io::stdout().flush()?;

    let stdin = std::io::stdin();

    let mut buf = String::with_capacity(100);
    stdin.read_line(&mut buf)?;
    Ok(Primitive::String(buf))
}
/// Api description
#[no_mangle]
pub fn api_description(
    _params: Vec<Primitive>,
    _compiler: Box<Compiler>,
) -> NativeFunctionCallResult {
    Ok(Primitive::Struct(BTreeMap::from([(
        "read_line".into(),
        Primitive::String(
            "read_line(string, string,...) -> [string], Read Line from stdin. Optional message(s)"
                .into(),
        ),
    )])))
}
