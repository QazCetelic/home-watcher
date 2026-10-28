// TODO store unix timestamp instead of the formatted date with new DB schema

#[derive(Clone, Debug, PartialEq)]
pub struct Date {
    year: usize,
    month: usize,
    day: usize,
}
impl Date {
    pub fn from_ymd(year: usize, month: usize, day: usize) -> Date {
        Date {
            year,
            month,
            day,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Time {
    hour: usize,
    minute: usize,
    second: usize,
}
impl Time {
    pub fn from_hms(hour: usize, minute: usize, second: usize) -> Time {
        Time {
            hour,
            minute,
            second,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DateTime {
    pub date: Date,
    pub time: Time,
}
impl DateTime {
    pub fn as_integer(&self) -> usize {
        (((((self.date.year * 100) + self.date.month) * 100 + self.date.day) * 100 + self.time.hour) * 100 + self.time.minute) * 100 + self.time.second
    }
    pub fn from_integer(integer: usize) -> DateTime {
        let mut parts = [0; 5];
        let mut tmp = integer;
        for part in parts.iter_mut().rev() {
            *part = tmp % 100;
            tmp /= 100;
        }
        DateTime {
            date: Date::from_ymd(tmp, parts[0], parts[1]),
            time: Time::from_hms(parts[2], parts[3], parts[4]),
        }
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