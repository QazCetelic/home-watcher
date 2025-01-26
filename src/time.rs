use std::str::FromStr;

#[derive(Clone)]
pub struct Date {
    year: usize,
    month: usize,
    day: usize,
}
impl Date {
    /// e.g. 2025-01-25
    pub(crate) fn from_str(str: &str) -> Option<Date> {
        let split = str.split("-").collect::<Vec<&str>>();
        Some(Date {
            year: usize::from_str(split.get(0)?).ok()?,
            month: usize::from_str(split.get(1)?).ok()?,
            day: usize::from_str(split.get(2)?).ok()?,
        })
    }
    pub fn from_ymd(year: usize, month: usize, day: usize) -> Date {
        Date {
            year,
            month,
            day,
        }
    }
    pub fn year(&self) -> usize { self.year }
    pub fn month(&self) -> usize { self.month }
    pub fn day(&self) -> usize { self.day }
    pub fn to_ymd_string(&self) -> String { format!("{:02}-{:02}-{:02}", self.year, self.month, self.day) }
}

#[derive(Clone)]
pub struct Time {
    hour: usize,
    minute: usize,
    second: usize,
}
impl Time {
    /// e.g. 12:55:03
    pub(crate) fn from_str(str: &str) -> Option<Time> {
        let split = str.split(":").collect::<Vec<&str>>();
        Some(Time {
            hour: usize::from_str(split.get(0)?).ok()?,
            minute: usize::from_str(split.get(1)?).ok()?,
            second: usize::from_str(split.get(2)?).ok()?,
        })
    }
    pub fn from_hms(hour: usize, minute: usize, second: usize) -> Time {
        Time {
            hour,
            minute,
            second,
        }
    }
    pub fn hour(&self) -> usize { self.hour }
    pub fn minute(&self) -> usize { self.minute }
    pub fn second(&self) -> usize { self.second }
    pub fn to_hms_string(&self) -> String { format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second) }
}

#[derive(Clone)]
pub struct DateTime {
    pub date: Date,
    pub time: Time,
}