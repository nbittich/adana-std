use std::collections::BTreeMap;

use adana_script_core::primitive::{Compiler, NativeFunctionCallResult, Primitive};
use chrono::{offset::Local, Datelike, NaiveDateTime, Timelike};
pub static DATE_FORMATS: [&str; 9] = [
    "%Y-%m-%dT%H:%M:%S%.3f%Z",
    "%Y-%m-%dT%H:%M:%S%Z",
    "%Y-%m-%d%:z",
    "%Y-%m-%d",
    "%d-%m-%Y",
    "%Y-%d-%m",
    "%m-%d-%Y",
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
            Primitive::Int(d.timestamp_millis() as i128),
        ),
        (
            "weekDay".into(),
            Primitive::String(date.weekday().to_string()),
        ),
        ("week".into(), Primitive::U8(d.iso_week().week() as u8)),
        ("day".into(), Primitive::U8(date.day() as u8)),
        ("month".into(), Primitive::U8(date.month() as u8)),
        ("year".into(), Primitive::U8(date.year() as u8)),
        ("hour".into(), Primitive::U8(time.hour() as u8)),
        ("minute".into(), Primitive::U8(time.minute() as u8)),
        ("second".into(), Primitive::U8(time.second() as u8)),
        ("leap_year".into(), Primitive::Bool(date.leap_year())),
    ]))
}

#[no_mangle]
fn parse(mut params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    if params.is_empty() {
        return Err(anyhow::anyhow!(
            "not enough parameter. at least a string should be provided."
        ));
    }

    let Primitive::String(s) = params.remove(1) else {
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

#[no_mangle]
pub fn now(_params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    let now = Local::now().naive_local();
    Ok(make_date_time_struct(&now))
}

#[cfg(test)]
mod test {
    use chrono::Local;

    use crate::make_date_time_struct;

    #[test]
    fn check_str() {
        let now = Local::now().naive_local();
        dbg!(make_date_time_struct(&now));
    }
}
