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
impl DateTime {
    pub fn as_integer(&self) -> usize {
        (((((self.date.year * 100) + self.date.month) * 100 + self.date.day) * 100 + self.time.hour) * 100 + self.time.minute) * 100 + self.time.second
    }
    pub fn from_integer(integer: usize) -> DateTime {
        let mut tmp = integer;
        let seconds = tmp % 100;
        tmp /= 100;
        let minutes = tmp % 100;
        tmp /= 100;
        let hours = tmp % 100;
        let time = Time::from_hms(hours, minutes, seconds);
        tmp /= 100;
        let days = tmp % 100;
        tmp /= 100;
        let months = tmp % 100;
        tmp /= 100;
        let years = tmp;
        let date = Date::from_ymd(years, months, days);
        DateTime { date, time }
    }
    pub fn to_string(&self) -> String { format!("{:02}-{:02}-{:02} {:02}:{:02}:{:02}", self.date.year, self.date.month, self.date.day, self.time.hour, self.time.minute, self.time.second) }
}

#[test]
fn test_datetime_conversion() {
    for year in (2000..=2020).step_by(5) {
        for month in (1..=12).step_by(3) {
            for day in 1..=28 {
                for hour in 0..=23 {
                    for minute in (0..=59).step_by(15) {
                        for second in (0..=59).step_by(15) {
                            let time = Time::from_hms(hour, minute, second);
                            let date = Date::from_ymd(year, month, day);
                            let dt = DateTime { date, time };
                            let as_int = dt.as_integer();
                            let dt2 = DateTime::from_integer(as_int);
                            let dt_s = dt.to_string();
                            let dt2_s = dt2.to_string();
                            assert_eq!(dt_s, dt2_s);
                        }
                    }
                }
            }
        }
    }
}