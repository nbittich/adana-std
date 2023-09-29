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
        ("week".into(), Primitive::Int(d.iso_week().week() as i128)),
        ("day".into(), Primitive::Int(date.day() as i128)),
        ("month".into(), Primitive::Int(date.month() as i128)),
        ("year".into(), Primitive::Int(date.year() as i128)),
        ("hour".into(), Primitive::Int(time.hour() as i128)),
        ("minute".into(), Primitive::Int(time.minute() as i128)),
        ("second".into(), Primitive::Int(time.second() as i128)),
        ("leap_year".into(), Primitive::Bool(date.leap_year())),
    ]))
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
