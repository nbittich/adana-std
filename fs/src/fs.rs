use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
};

use adana_script_core::primitive::{Compiler, NativeFunctionCallResult, Primitive};
use anyhow::Context;

fn get_file_from_params(
    params: &[Primitive],
    param_len: usize,
    open_option: &mut OpenOptions,
    fail_if_not_exist: bool,
    open_file: bool,
) -> anyhow::Result<(PathBuf, Option<File>)> {
    if params.len() != param_len {
        return Err(anyhow::anyhow!(
            "too many / not enough argument(s). expected argument count: {param_len}"
        ));
    }
    let v = &params[0];
    match v {
        Primitive::String(file_path) => {
            let path_buf = PathBuf::from(file_path);

            if !path_buf.exists() {
                if fail_if_not_exist {
                    Err(anyhow::anyhow!("file {file_path} not found"))
                } else {
                    Ok((path_buf, None))
                }
            } else {
                if !open_file {
                    Ok((path_buf, None))
                } else {
                    let file = open_option.open(file_path)?;
                    Ok((path_buf, Some(file)))
                }
            }
        }
        _ => Err(anyhow::anyhow!("wrong read lines call".to_string())),
    }
}

fn _write(params: &[Primitive], open_options: &mut OpenOptions) -> NativeFunctionCallResult {
    let (_, file) = get_file_from_params(params, 2, open_options, false, true)?;
    let mut writer = BufWriter::new(file.context("file could not be opened")?);
    let input = &params[1];
    match input {
        Primitive::String(s) => writer.write_all(s.as_bytes())?,

        _ => writer.write_all(input.to_string().as_bytes())?,
    }
    Ok(Primitive::Unit)
}

#[no_mangle]
pub fn read_file(params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    let mut open_options = OpenOptions::new();
    let open_options = open_options.read(true).write(false);
    let (pb, file) = get_file_from_params(&params, 1, open_options, true, true)?;
    let file = file.context("file not found")?;
    if !pb.is_file() {
        return Err(anyhow::anyhow!("Not a file"));
    }
    let reader = BufReader::new(file);
    Ok(Primitive::Array(
        reader
            .lines()
            .map(|s| s.map(Primitive::String))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

#[no_mangle]
pub fn read_dir(params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    let mut open_options = OpenOptions::new();
    let open_options = open_options.read(true).write(false);
    let (pb, _) = get_file_from_params(&params, 1, open_options, true, false)?;
    if !pb.is_dir() {
        return Err(anyhow::anyhow!("Not a directory"));
    }
    let dir = std::fs::read_dir(pb)?;

    let mut arr = vec![];

    for d in dir {
        let d = d?;
        arr.push(Primitive::String(
            d.path()
                .to_str()
                .context("path could not be created")?
                .to_string(),
        ));
    }
    Ok(Primitive::Array(arr))
}

#[no_mangle]
pub fn delete_file(params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    let mut open_options = OpenOptions::new();
    let open_options = open_options.read(true).write(false);
    let (pb, _) = get_file_from_params(&params, 1, open_options, true, false)?;
    std::fs::remove_file(pb)?;

    Ok(Primitive::Unit)
}

#[no_mangle]
pub fn path_exists(params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    let mut open_options = OpenOptions::new();
    let open_options = open_options.read(true).write(false);
    let (pb, _) = get_file_from_params(&params, 1, open_options, false, false)?;

    Ok(Primitive::Bool(pb.exists()))
}

#[no_mangle]
pub fn delete_empty_dir(
    params: Vec<Primitive>,
    _compiler: Box<Compiler>,
) -> NativeFunctionCallResult {
    let mut open_options = OpenOptions::new();
    let open_options = open_options.read(true).write(false);
    let (pb, _) = get_file_from_params(&params, 1, open_options, true, false)?;

    std::fs::remove_dir(pb)?;

    Ok(Primitive::Unit)
}

#[no_mangle]
pub fn delete_dir_all(
    params: Vec<Primitive>,
    _compiler: Box<Compiler>,
) -> NativeFunctionCallResult {
    let mut open_options = OpenOptions::new();
    let open_options = open_options.read(true).write(false);
    let (pb, _) = get_file_from_params(&params, 1, open_options, true, false)?;

    std::fs::remove_dir_all(pb)?;

    Ok(Primitive::Unit)
}

#[no_mangle]
pub fn write_file(params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    let mut open_options = OpenOptions::new();
    let open_options = open_options.write(true);
    _write(&params, open_options)
}

#[no_mangle]
pub fn append_file(params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    let mut open_options = OpenOptions::new();
    let open_options = open_options.append(true);
    _write(&params, open_options)
}

#[no_mangle]
pub fn rename_file_or_directory(
    params: Vec<Primitive>,
    _compiler: Box<Compiler>,
) -> NativeFunctionCallResult {
    let mut open_options = OpenOptions::new();

    let open_options = open_options.read(true);

    let (src, _) = get_file_from_params(&params, 2, open_options, true, false)?;
    let (dest, _) = get_file_from_params(&params, 2, open_options, false, false)?;
    std::fs::rename(src, dest)?;
    Ok(Primitive::Unit)
}

#[no_mangle]
pub fn fd_stats(params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    use std::time::UNIX_EPOCH;
    let mut open_options = OpenOptions::new();
    let open_options = open_options.read(true).write(false);

    let (pb, file) = get_file_from_params(&params, 1, open_options, false, true)?;

    let mut struc = BTreeMap::from([
        ("exists".into(), Primitive::Bool(pb.exists())),
        ("is_relative".into(), Primitive::Bool(pb.is_relative())),
        ("is_absolute".into(), Primitive::Bool(pb.is_absolute())),
        (
            "extension".into(),
            pb.extension()
                .and_then(|p| p.to_str())
                .map(|p| Primitive::String(p.to_string()))
                .unwrap_or_else(|| Primitive::Null),
        ),
        (
            "file_name".into(),
            pb.file_name()
                .and_then(|p| p.to_str())
                .map(|p| Primitive::String(p.to_string()))
                .unwrap_or_else(|| Primitive::Null),
        ),
    ]);
    if let Some(file) = file {
        let metadata = file.metadata()?;
        struc.extend(
            [
                ("len".into(), Primitive::Int(metadata.len() as i128)),
                (
                    "created".into(),
                    Primitive::Int(
                        metadata.created()?.duration_since(UNIX_EPOCH)?.as_millis() as i128
                    ),
                ),
                (
                    "modified".into(),
                    Primitive::Int(
                        metadata.modified()?.duration_since(UNIX_EPOCH)?.as_millis() as i128
                    ),
                ),
                ("is_file".into(), Primitive::Bool(metadata.is_file())),
                ("is_dir".into(), Primitive::Bool(metadata.is_dir())),
                ("is_symlink".into(), Primitive::Bool(metadata.is_symlink())),
            ],
        );
    }

    Ok(Primitive::Struct(struc))
}

/// Api description
#[no_mangle]
pub fn fs_description(
    _params: Vec<Primitive>,
    _compiler: Box<Compiler>,
) -> NativeFunctionCallResult {
    Ok(Primitive::Struct(BTreeMap::from([
        (
            "read_file".into(),
            Primitive::String("read_file(file_path) -> [string], Read lines in file".into()),
        ),
        (
            "fd_stats".into(),
            Primitive::String("fd_stats(file_path) -> struct, Stats about the file".into()),
        ),
        (
            "write_file".into(),
            Primitive::String("write_file(file_path, content), Write content to file".into()),
        ),
        (
            "delete_file".into(),
            Primitive::String("delete_file(file_path), Delete a file".into())
        ),
        (
            "delete_empty_dir".into(),
            Primitive::String("delete_empty_dir(path), Delete an empty directory".into())
        ),
        (
            "delete_dir_all".into(),
            Primitive::String("delete_dir_all(path), Delete a directory and all its content.".into())
        ),
        (
            "append_file".into(),
            Primitive::String("append_file(file_path, content), Append content to file".into()),
        ),
        (
            "path_exists".into(),
             Primitive::String("path_exists(file_path)-> bool, weither a path exists or not".into()),

        ),
        (
            "rename_file_or_directory".into(),
            Primitive::String("rename_file_or_directory(src_path, dest_path), Rename file".into()),
        ),
        (
            "read_dir".into(),
            Primitive::String(
                "read_dir(path) -> [string], Read directory, returning the full path for each entries".into(),
            ),
        ),
    ])))
}
