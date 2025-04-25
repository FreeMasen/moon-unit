use moon_unit::{Client, OneDayArgs, PhaseArgs};

#[tokio::main]
async fn main() {
    let client = Client::default();
    let now = time::OffsetDateTime::now_utc();
    let data = client
        .one_day(
            &OneDayArgs::builder()
                .year(now.year() as _)
                .month(now.month().into())
                .day(now.day())
                .tz(0.0)
                .lat(43.9033)
                .long(-91.6401)
                .build(),
        )
        .await
        .unwrap();
    println!("{data:#?}");
    let current_year = client
        .phases(&PhaseArgs::year(now.year() as _))
        .await
        .unwrap();
    println!("{current_year:#?}");
    let next_10 = client
        .phases(
            &PhaseArgs::build_by_date()
                .day(now.day())
                .month(now.month().into())
                .year(now.year() as _)
                .count(10)
                .build()
                .unwrap(),
        )
        .await
        .unwrap();
    println!("{next_10:#?}");
}
