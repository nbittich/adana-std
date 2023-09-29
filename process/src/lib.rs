use std::{collections::BTreeMap, io::Write, thread::JoinHandle, time::Duration};

use adana_script_core::{
    primitive::{Compiler, NativeFunctionCallResult, Primitive},
    Value,
};

#[no_mangle]
pub fn environ(params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    if params.is_empty() {
        let s = std::env::vars()
            .map(|(k, v)| (k, Primitive::String(v)))
            .collect::<BTreeMap<_, _>>();
        Ok(Primitive::Struct(s))
    } else {
        let r = std::env::var(params[0].to_string())
            .ok()
            .map(Primitive::String)
            .unwrap_or_else(|| Primitive::Null);
        Ok(r)
    }
}
#[no_mangle]
pub fn delay(mut params: Vec<Primitive>, mut compiler: Box<Compiler>) -> NativeFunctionCallResult {
    if params.is_empty() {
        Err(anyhow::anyhow!("at least one parameter must be provided"))
    } else {
        let Primitive::Int(delay) = params.remove(0) else {
            return Err(anyhow::anyhow!(
                "first parameter must be the sleep duration (int)"
            ));
        };
        if params.is_empty() {
            std::thread::sleep(Duration::from_millis(delay as u64));
            Ok(Primitive::Unit)
        } else if params.len() == 2 {
            let handle: JoinHandle<anyhow::Result<()>> = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(delay as u64));

                let f @ Primitive::Function { .. } = params.remove(0) else {
                    return Err(anyhow::anyhow!("second parameter must be a function"));
                };
                let Primitive::Struct(ctx) = params.remove(0) else {
                    return Err(anyhow::anyhow!(
                        "third parameter must be the context (struct)"
                    ));
                };
                let function = f.to_value()?;
                let ctx = ctx
                    .into_iter()
                    .map(|(k, v)| (k, v.ref_prim()))
                    .collect::<BTreeMap<_, _>>();

                let parameters = ctx.keys().cloned().map(Value::String).collect::<Vec<_>>();
                compiler(
                    Value::FunctionCall {
                        parameters: Box::new(Value::BlockParen(parameters)),
                        function: Box::new(function),
                    },
                    ctx,
                )?;
                std::io::stdout().flush()?;
                Ok(())
            });
            std::thread::spawn(move || match handle.join() {
                Ok(Ok(())) => {}
                e => eprintln!("{e:?}"),
            });
            Ok(Primitive::Unit)
        } else {
            Err(anyhow::anyhow!("too many arguments"))
        }
    }
}
/// Api description
#[no_mangle]
pub fn api_description(
    _params: Vec<Primitive>,
    _compiler: Box<Compiler>,
) -> NativeFunctionCallResult {
    Ok(Primitive::Struct(BTreeMap::from([(
        "environ".into(),
        Primitive::String(
            "environ(string) -> struct | string, takes an optional key, return environment variable(s)"
                .into(),
        )),
        ("delay".into(),
        Primitive::String(
            r#"delay(int, function, ctx) -> () | sleep for a specified amount of time. 
            Takes optional function and immutable context (struct) as callback.
            e.g : 
               f = () => {
                    e = process.environ("WEZTERM_PANE")
                    if(e!= null) {
                        println("found env: " + e)
                    }
               }
               s = struct {}
               process.delay(1000, f, s)
               "#
            .into(),
        )),
    ])))
}
