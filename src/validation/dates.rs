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

// Data sem hora no formato AAAA-MM-DD (coluna `date` do Postgres).
pub fn serialize_date<S: Serializer>(date: &time::Date, serializer: S) -> Result<S::Ok, S::Error> {
    let format = format_description!("[year]-[month]-[day]");
    let text = date.format(&format).map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&text)
}

// Lê uma data AAAA-MM-DD estrita (como o z.iso.date() do Zod): 4 dígitos, hífen, 2 dígitos, hífen, 2 dígitos, e
// a data precisa existir (2026-02-30 não existe). Escrita à mão, sem depender das variações que o `time` aceita.
pub fn parse_iso_date(text: &str) -> Option<time::Date> {
    let bytes = text.as_bytes();
    let shape_ok = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    if !shape_ok {
        return None;
    }
    // Com o formato conferido, cada pedaço é só dígitos: o `parse` não falha.
    let year: i32 = text[0..4].parse().ok()?;
    let month: u8 = text[5..7].parse().ok()?;
    let day: u8 = text[8..10].parse().ok()?;
    let month = time::Month::try_from(month).ok()?; // 13 não é mês
    time::Date::from_calendar_date(year, month, day).ok() // dia que não existe no mês → None
}

#[cfg(test)]
mod tests {
    use super::parse_iso_date;

    #[test]
    fn datas() {
        assert!(parse_iso_date("2024-02-29").is_some()); // ano bissexto
        for invalida in [
            "2026-02-30",
            "2026-13-01",
            "29/09/2026",
            "",
            "2026-9-01",
            "2026-09-01T00:00:00Z",
            "+026-09-01",
        ] {
            assert!(parse_iso_date(invalida).is_none(), "{invalida}");
        }
    }
}
