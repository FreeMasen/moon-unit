use std::ops::Rem;

use serde::{Deserialize, Deserializer, Serialize};
use time::{Date, OffsetDateTime, PrimitiveDateTime};

type Result<T = (), E = anyhow::Error> = core::result::Result<T, E>;

pub struct Client {
    inner: reqwest::Client,
    base_url: String,
}
const DEFAULT_BASE_URL: &str = "https://aa.usno.navy.mil";

impl Default for Client {
    fn default() -> Self {
        Self::new(reqwest::Client::default(), DEFAULT_BASE_URL)
    }
}

impl From<reqwest::Client> for Client {
    fn from(value: reqwest::Client) -> Self {
        Self::new(value, DEFAULT_BASE_URL)
    }
}

impl Client {
    pub fn with_base_url(base_url: impl ToString) -> Self {
        Self::new(Default::default(), base_url)
    }

    pub fn new(client: reqwest::Client, base_url: impl ToString) -> Self {
        Self {
            inner: client,
            base_url: base_url.to_string(),
        }
    }

    pub async fn one_day(&self, query: &OneDayArgs) -> Result<OneDay> {
        self.inner
            .get(format!("{}/api/rstt/oneday", self.base_url))
            .query(query)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send request: {e}"))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("invalid status in response: {e}"))?
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("failed to deserialize response: {e}"))
    }

    pub async fn phases(&self, query: &PhaseArgs) -> Result<MoonPhasesResponse> {
        let path = if matches!(query, PhaseArgs::Year { .. }) {
            "year"
        } else {
            "date"
        };
        self.inner
            .get(format!("{}/api/moon/phases/{path}", self.base_url))
            .query(query)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send request: {e}"))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("invalid status in response: {e}"))?
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("failed to deserialize response: {e}"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneDayArgs {
    date: String,
    coords: String,
    tz: f32,
}

#[bon::bon]
impl OneDayArgs {
    #[builder]
    pub fn new(year: u16, month: u8, day: u8, lat: f32, long: f32, tz: f32) -> Self {
        Self {
            date: format!("{year:04}-{month:02}-{day:02}"),
            coords: format!("{lat:.04},{long:.04}"),
            tz,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PhaseArgs {
    Year { year: u16 },
    ByDate { date: String, nump: u16 },
}

#[bon::bon]
impl PhaseArgs {
    pub fn year(year: u16) -> Self {
        Self::Year { year: year }
    }

    #[builder(
        start_fn = build_by_date,
        finish_fn = build,
    )]
    pub fn by_date(year: u16, month: u8, day: u8, count: u16) -> Result<Self> {
        if count < 1 || count > 99 {
            anyhow::bail!("Invalid count, must be between 1 and 99 inclusive found: {count}")
        }
        Ok(Self::ByDate {
            date: format!("{year:04}-{month:02}-{day:02}"),
            nump: count,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneDay {
    pub properties: OneDayProps,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneDayProps {
    pub data: OneDayData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneDayData {
    #[serde(alias = "closestphase")]
    pub closest_phase: ClosestPhase,
    #[serde(alias = "curphase")]
    pub current_phase: MoonPhase,
    pub day_of_week: String,
    #[serde(alias = "fracillum")]
    #[serde(deserialize_with = "deser_fracillum")]
    pub percent_illuminated: u8,
    #[serde(alias = "moondata")]
    pub moon_data: Vec<CelestialEvent>,
    #[serde(alias = "sundata")]
    pub sun_data: Vec<CelestialEvent>,
    month: u8,
    day: u8,
    year: u16,
    tz: f32,
}

impl OneDayData {
    pub fn when(&self) -> Result<OffsetDateTime> {
        let month = time::Month::try_from(self.month).map_err(|e| {
            anyhow::anyhow!("Invalid month in date: {e}")
        })?;
        let dt = Date::from_calendar_date(self.year as _, month, self.day).map_err(|e| {
            anyhow::anyhow!("invalid date: {e}")
        })?;
        let time = time::Time::MIDNIGHT;
        let tz_hour = self.tz.floor() as i8;
        let tz_minute = (self.tz.rem(1.0) * 60.0) as i8;
        let tz = time::UtcOffset::from_hms(tz_hour, tz_minute, 0).unwrap_or(time::UtcOffset::UTC);
        Ok(OffsetDateTime::new_in_offset(dt, time, tz))
    }

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosestPhase {
    day: u8,
    month: u8,
    year: u16,
    #[serde(deserialize_with = "deser_time")]
    time: Time,
    pub phase: MoonPhase,
}

impl ClosestPhase {
    pub fn when(&self) -> Result<PrimitiveDateTime> {
        let month = time::Month::try_from(self.month).map_err(|e| {
            anyhow::anyhow!("Invalid month in date: {e}")
        })?;
        let dt = Date::from_calendar_date(self.year as _, month, self.day).map_err(|e| {
            anyhow::anyhow!("invalid date: {e}")
        })?;
        let t = time::Time::from_hms(self.time.hour, self.time.minute, 0).map_err(|e| {
            anyhow::anyhow!("invalid time: {e}")
        })?;
        Ok(PrimitiveDateTime::new(dt, t))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MoonPhase {
    #[serde(alias = "New Moon")]
    New,
    #[serde(alias = "Waxing Crescent")]
    WaxingCrescent,
    #[serde(alias = "First Quarter")]
    FirstQuarter,
    #[serde(alias = "Waxing Gibbous")]
    WaxingGibbous,
    #[serde(alias = "Full Moon")]
    Full,
    #[serde(alias = "Waning Gibbous")]
    WaningGibbous,
    #[serde(alias = "Last Quarter")]
    LastQuarter,
    #[serde(alias = "Waning Crescent")]
    WaningCrescent,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Phenomenon {
    Rise,
    #[serde(alias = "Upper Transit")]
    Apex,
    #[serde(alias = "Begin Civil Twilight")]
    TwilightBegins,
    Set,
    #[serde(alias = "End Civil Twilight")]
    TwilightEnds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CelestialEvent {
    #[serde(alias = "phen")]
    pub phenomenon: Phenomenon,
    #[serde(deserialize_with = "deser_time")]
    time: Time,
}

impl CelestialEvent {
    pub fn when(&self) -> Result<time::Time> {
        time::Time::from_hms(self.time.hour, self.time.minute, 0).map_err(|e| {
            anyhow::anyhow!("invalid time: {e}")
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Time {
    hour: u8,
    minute: u8,
}

fn deser_fracillum<'de, D>(d: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    struct FracillumVisitor;
    impl<'de> serde::de::Visitor<'de> for FracillumVisitor {
        type Value = u8;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str(r"time with the format \d{2}:\d{2}")
        }
        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            v.trim_end_matches('%').parse().map_err(|e| {
                serde::de::Error::custom(format!("Failed ot parse precent: {e}\n\t{v:?}"))
            })
        }

        fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v)
        }
    }
    d.deserialize_any(FracillumVisitor)
}

fn deser_time<'de, D>(d: D) -> Result<Time, D::Error>
where
    D: Deserializer<'de>,
{
    struct TimeVisitor;
    impl<'de> serde::de::Visitor<'de> for TimeVisitor {
        type Value = Time;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("number and percent")
        }
        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let (hours, minutes) = v
                .split_once(":")
                .ok_or_else(|| serde::de::Error::custom(format!("time missing colon: {v:?}")))?;
            Ok(Time {
                hour: hours
                    .parse()
                    .map_err(|e| serde::de::Error::custom(format!("invalid hour-{e}: {v:?}")))?,
                minute: minutes
                    .parse()
                    .map_err(|e| serde::de::Error::custom(format!("invalid minute-{e}: {v:?}")))?,
            })
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut hour = None;
            let mut minute = None;
            while let Some(key) = map.next_key::<&str>()? {
                match key {
                    "hour" => {
                        hour = Some(map.next_value::<u8>()?);
                    }
                    "minute" => {
                        minute = Some(map.next_value::<u8>()?);
                    }
                    _ => {}
                }
            }
            let hour = hour.ok_or_else(|| serde::de::Error::custom("hour missing from map"))?;
            let minute =
                minute.ok_or_else(|| serde::de::Error::custom("minute missing from map"))?;
            Ok(Time { hour, minute })
        }
    }
    d.deserialize_any(TimeVisitor)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoonPhasesResponse {
    #[serde(alias = "numphases")]
    pub count: u16,
    #[serde(alias = "phasedata")]
    pub phases: Vec<MoonPhaseEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoonPhaseEntry {
    pub phase: MoonPhase,
    day: u8,
    month: u8,
    year: u16,
    #[serde(alias = "deser_time")]
    time: Time,
}

impl MoonPhaseEntry {
    pub fn when(&self) -> Result<PrimitiveDateTime> {
        let month = time::Month::try_from(self.month).map_err(|e| {
            anyhow::anyhow!("Invalid month in date: {e}")
        })?;
        let dt = Date::from_calendar_date(self.year as _, month, self.day).map_err(|e| {
            anyhow::anyhow!("invalid date: {e}")
        })?;
        let t = time::Time::from_hms(self.time.hour, self.time.minute, 0).map_err(|e| {
            anyhow::anyhow!("invalid time: {e}")
        })?;
        Ok(PrimitiveDateTime::new(dt, t))
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_day_args() {
        insta::assert_json_snapshot!(OneDayArgs::builder()
            .year(2025)
            .month(4)
            .day(25)
            .tz(0.0)
            .lat(0.0)
            .long(0.0)
            .build())
    }

    #[test]
    fn phases_args() {
        insta::assert_json_snapshot!(&[
            PhaseArgs::year(2025),
            PhaseArgs::build_by_date()
                .year(2025)
                .month(4)
                .day(25)
                .count(8)
                .build()
                .unwrap(),
        ])
    }
}
