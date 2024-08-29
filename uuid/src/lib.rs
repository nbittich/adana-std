use adana_script_core::{
    primitive::{Compiler, NativeFunctionCallResult},
    Primitive,
};

#[no_mangle]
pub fn new(_params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    let uuid = uuid::Uuid::new_v4();
    Ok(Primitive::String(uuid.to_string()))
}
