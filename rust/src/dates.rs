// Datas no mesmo formato do JavaScript (`Date.toISOString()`): UTC, milissegundos e "Z".
// Ex.: 2026-10-05T12:34:56.789Z. O Postgres guarda microssegundos; o TS mostra só os milissegundos.
use serde::Serializer;
use time::{OffsetDateTime, UtcOffset, macros::format_description};

pub fn serialize_js_iso<S: Serializer>(
    date: &OffsetDateTime,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let format =
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");
    let text = date
        .to_offset(UtcOffset::UTC)
        .format(&format)
        .map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&text)
}
