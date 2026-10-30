//! Source local album ordering over validated Subsonic-normalized rows.
use feishin_domain::*;
use std::cmp::Ordering;

pub(super) struct Row {
    pub album: Album,
    // Preserve source float ordering independently of integer-ms representation.
    pub duration_ms: f64,
    pub missing_song_count: bool,
}
#[derive(Clone, PartialEq)]
enum Key {
    Number(f64),
    Text(String),
    Null,
    Missing,
    InvalidDate,
}
fn compare(a: &Key, b: &Key) -> Ordering {
    use Key::*;
    match (a, b) {
        (Missing, Missing) | (Null, Null) | (InvalidDate, InvalidDate) => Ordering::Equal,
        (InvalidDate, _) => Ordering::Greater,
        (_, InvalidDate) => Ordering::Less,
        (Missing, _) => Ordering::Greater,
        (_, Missing) => Ordering::Less,
        (Null, _) => Ordering::Greater,
        (_, Null) => Ordering::Less,
        (Number(a), Number(b)) => a.total_cmp(b),
        (Text(a), Text(b)) => a.encode_utf16().cmp(b.encode_utf16()),
        _ => unreachable!("one sort uses homogeneous scalar keys"),
    }
}
fn number(value: Option<f64>) -> Key {
    value
        .map(|n| Key::Number(if n == 0.0 { 0.0 } else { n }))
        .unwrap_or(Key::Null)
}
fn days(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468
}
fn clipped(year: i64, month: i64, day: i64) -> Key {
    let days = days(year, month, day);
    if (-100_000_000..=100_000_000).contains(&days) {
        Key::Number(days as f64)
    } else {
        Key::InvalidDate
    }
}
// Only formats emitted by the checked Subsonic normalizer are accepted here.
// Legacy integer Date.parse rules are pinned against UTC; see ADR-023 for the
// explicit timezone policy and stable ordering of invalid date ties.
fn date(value: Option<&str>, year: Option<i32>) -> Key {
    if let Some(value) = value.filter(|v| !v.is_empty()) {
        let b = value.as_bytes();
        if b.len() == 10
            && b[4] == b'-'
            && b[7] == b'-'
            && b.iter()
                .enumerate()
                .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
        {
            let y = value[..4].parse::<i64>();
            let m = value[5..7].parse::<i64>();
            let d = value[8..].parse::<i64>();
            return match (y, m, d) {
                (Ok(y), Ok(m), Ok(d)) if (1..=12).contains(&m) && (1..=31).contains(&d) => {
                    clipped(y, m, d)
                }
                _ => Key::InvalidDate,
            };
        }
        let Ok(y) = value.parse::<i32>() else {
            return Key::InvalidDate;
        };
        if y <= -100000 && value.len() == 7 {
            return clipped(i64::from(y), 1, 1);
        }
        let y = i64::from(y).abs();
        return match y {
            0 => clipped(2000, 1, 1),
            1..=12 => clipped(2001, y, 1),
            13..=31 => Key::InvalidDate,
            32..=49 => clipped(2000 + y, 1, 1),
            50..=99 => clipped(1900 + y, 1, 1),
            _ => clipped(y, 1, 1),
        };
    }
    match year.filter(|y| *y != 0) {
        Some(y) => clipped(
            if (0..=99).contains(&y) {
                1900 + i64::from(y)
            } else {
                i64::from(y)
            },
            1,
            1,
        ),
        None => Key::Number(0.0),
    }
}
pub(super) fn sorted(
    mut rows: Vec<Row>,
    sort: AlbumListSort,
    order: SortOrder,
    random: &mut dyn Random,
) -> Vec<Row> {
    if sort == AlbumListSort::Id {
        if order == SortOrder::Desc {
            rows.reverse();
        }
        return rows;
    }
    if sort == AlbumListSort::Random {
        for index in 0..rows.len() {
            let unit = (random.next_u64() >> 11) as f64 / 9_007_199_254_740_992.0;
            let remaining = rows.len() - index;
            let offset = ((remaining as f64 * unit).floor() as usize).min(remaining - 1);
            rows.swap(index, index + offset);
        }
        return rows;
    }
    if matches!(
        sort,
        AlbumListSort::Artist
            | AlbumListSort::CommunityRating
            | AlbumListSort::CriticRating
            | AlbumListSort::ExplicitStatus
            | AlbumListSort::Favorited
            | AlbumListSort::PlayCount
            | AlbumListSort::RecentlyPlayed
    ) {
        return rows;
    }
    let mut keyed: Vec<_> = rows
        .into_iter()
        .map(|row| {
            let a = &row.album;
            let keys = match sort {
                // Source albumArtist key is absent, so its secondary name asc wins.
                AlbumListSort::AlbumArtist => vec![Key::Text(a.name.to_lowercase())],
                AlbumListSort::Name => vec![Key::Text(a.name.to_lowercase())],
                AlbumListSort::Duration => vec![number(Some(row.duration_ms))],
                AlbumListSort::Rating => vec![number(a.user_rating)],
                AlbumListSort::RecentlyAdded => vec![Key::Text(a.created_at.clone())],
                AlbumListSort::ReleaseDate => vec![
                    date(a.original_date.as_deref(), a.original_year),
                    date(a.release_date.as_deref(), a.release_year),
                    Key::Text(a.name.to_lowercase()),
                ],
                AlbumListSort::SongCount => vec![if row.missing_song_count {
                    Key::Missing
                } else {
                    number(a.song_count.map(|v| v as f64))
                }],
                AlbumListSort::SortName => vec![Key::Text(a.sort_name.clone())],
                AlbumListSort::Year => vec![number(a.release_year.map(f64::from))],
                _ => vec![],
            };
            (keys, row)
        })
        .collect();
    keyed.sort_by(|(a, _), (b, _)| {
        for (index, (a, b)) in a.iter().zip(b).enumerate() {
            // Lodash's NaN/NaN comparator is inconsistent. Use a total order
            // with stable invalid ties, without falling through to name keys.
            if matches!((a, b), (Key::InvalidDate, Key::InvalidDate)) {
                return Ordering::Equal;
            }
            let value = compare(a, b);
            if value != Ordering::Equal {
                let descending = order == SortOrder::Desc
                    && sort != AlbumListSort::AlbumArtist
                    && !(sort == AlbumListSort::ReleaseDate && index == 2);
                return if descending { value.reverse() } else { value };
            }
        }
        Ordering::Equal
    });
    keyed.into_iter().map(|(_, row)| row).collect()
}
