use adana_script_core::primitive::{Compiler, NativeFunctionCallResult, Primitive};
use anyhow::Context;
use chrono::DateTime;
use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike, offset::Local};
use std::collections::BTreeMap;
use std::fmt::Write;

pub static DATE_FORMATS: [&str; 8] = [
    "%d/%m/%Y %H:%M:%S",
    "%Y-%m-%d %H:%M:%S",
    "%d-%m-%Y %H:%M:%S",
    "%Y-%m-%d",
    "%d-%m-%Y",
    "%Y-%d-%m",
    "%m/%d/%Y",
    "%d/%m/%Y",
];
pub static TIME_FORMATS: [&str; 4] = ["%H:%M:%S%.3f%Z", "%H:%M:%S%Z", "%H:%M:%S", "%H:%M"];

fn make_date_time_struct(d: &NaiveDateTime) -> Primitive {
    let date = d.date();
    let time = d.time();

    Primitive::Struct(BTreeMap::from([
        (
            "timestamp".into(),
            Primitive::Int(d.and_utc().timestamp_millis() as i128),
        ),
        (
            "weekDay".into(),
            Primitive::String(date.weekday().to_string()),
        ),
        ("week".into(), Primitive::U8(d.iso_week().week() as u8)),
        ("day".into(), Primitive::U8(date.day() as u8)),
        ("month".into(), Primitive::U8(date.month() as u8)),
        ("year".into(), Primitive::Int(date.year() as i128)),
        ("hour".into(), Primitive::U8(time.hour() as u8)),
        ("minute".into(), Primitive::U8(time.minute() as u8)),
        ("second".into(), Primitive::U8(time.second() as u8)),
        ("leap_year".into(), Primitive::Bool(date.leap_year())),
    ]))
}

#[unsafe(no_mangle)]
fn from(mut params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    if params.len() < 3 {
        return Err(anyhow::anyhow!(
            "not enough parameters. at least year, month, day must be provided"
        ));
    }
    let get_i32_from_prim = |prim| match prim {
        Primitive::I8(n) => Ok(n as i32),
        Primitive::U8(n) => Ok(n as i32),
        Primitive::Int(n) => Ok(n as i32),
        _ => Err(anyhow::anyhow!("not an int")),
    };
    let year = get_i32_from_prim(params.remove(0))?;
    let month = get_i32_from_prim(params.remove(0))? as u32;
    let day = get_i32_from_prim(params.remove(0))? as u32;

    let date = {
        let date = NaiveDate::from_ymd_opt(year, month, day).context("could not extract date")?;

        if params.len() == 3 {
            let hour = get_i32_from_prim(params.remove(0))? as u32;
            let minute = get_i32_from_prim(params.remove(0))? as u32;
            let second = get_i32_from_prim(params.remove(0))? as u32;
            date.and_hms_opt(hour, minute, second)
        } else {
            date.and_hms_opt(0, 0, 0)
        }
    }
    .context("could not make date")?;
    Ok(make_date_time_struct(&date))
}

#[unsafe(no_mangle)]
fn format(mut params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    if params.is_empty() {
        return Err(anyhow::anyhow!(
            "not enough parameter. at least a timestamp should be provided."
        ));
    }

    let Primitive::Int(s) = params.remove(0) else {
        return Err(anyhow::anyhow!(
            "first parameter should be the timestamp (int)"
        ));
    };

    let date =
        DateTime::from_timestamp_millis(s as i64).context("could not convert timestamp to date")?;
    if !params.is_empty() {
        let Primitive::String(ref f) = params.remove(0) else {
            return Err(anyhow::anyhow!(
                "second parameter (optional) should be the format as string"
            ));
        };
        let mut res = String::new();
        write!(res, "{}", date.format(f))?;
        Ok(Primitive::String(res))
    } else {
        let mut res = String::new();
        write!(res, "{}", date.format(DATE_FORMATS[0]))?;
        Ok(Primitive::String(res))
    }
}

#[unsafe(no_mangle)]
fn parse(mut params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    if params.is_empty() {
        return Err(anyhow::anyhow!(
            "not enough parameter. at least a string should be provided."
        ));
    }

    let Primitive::String(s) = params.remove(0) else {
        return Err(anyhow::anyhow!(
            "first parameter should be the date formatted as a string"
        ));
    };

    if !params.is_empty() {
        let Primitive::String(ref f) = params.remove(0) else {
            return Err(anyhow::anyhow!(
                "second parameter (optional) should be the format as string"
            ));
        };
        let date = NaiveDateTime::parse_from_str(s.as_str(), f)?;
        Ok(make_date_time_struct(&date))
    } else {
        let mut date = None;
        for format in DATE_FORMATS {
            match NaiveDateTime::parse_from_str(s.as_str(), format) {
                Ok(d) => {
                    date = Some(d);
                    break;
                }
                Err(_e) => {}
            }
        }
        if let Some(date) = date {
            Ok(make_date_time_struct(&date))
        } else {
            Err(anyhow::anyhow!("could not determine date format. {s}"))
        }
    }
}

#[unsafe(no_mangle)]
pub fn now(_params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    let now = Local::now().naive_local();
    Ok(make_date_time_struct(&now))
}

/// Api description
#[unsafe(no_mangle)]
pub fn api_description(
    _params: Vec<Primitive>,
    _compiler: Box<Compiler>,
) -> NativeFunctionCallResult {
    Ok(Primitive::Struct(BTreeMap::from([
        (
            "from".into(),
            Primitive::String(
                r#"from(year, month, day, [hour, min, sec]) -> struct | 
                construct a date struct from year month day"#
                    .into(),
            ),
        ),
        (
            "format".into(),
            Primitive::String(
                "format(timestamp_millis, [format]) -> string | format a timestamp".into(),
            ),
        ),
        (
            "parse".into(),
            Primitive::String(
                r#"parse(date_str, [format]) -> struct | 
            parse a date string. optional format can be provided"#
                    .into(),
            ),
        ),
        (
            "now".into(),
            Primitive::String("now() -> struct | return current date struct ".into()),
        ),
    ])))
}

#[cfg(test)]
mod test {
    use adana_script_core::primitive::Primitive;
    use chrono::Local;

    use crate::format;

    #[test]
    fn check_str() {
        let now = Local::now().naive_local();
        let r = format(
            vec![Primitive::Int(now.and_utc().timestamp_millis() as i128)],
            Box::new(|_, _| Ok(Primitive::Unit)),
        )
        .unwrap();
        dbg!(r);
    }
}
