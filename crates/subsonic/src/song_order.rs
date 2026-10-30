//! Validated source-local song ordering; no platform date or RNG dependency.
use crate::ExchangeError;
use feishin_domain::*;
use std::cmp::Ordering;

pub(super) struct Row {
    pub song: Song,
    pub duration_ms: f64,
}
enum Key {
    Number(f64),
    Text(String),
    Bool(bool),
    Null,
    Missing,
}
impl Key {
    fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Missing, Self::Missing) | (Self::Null, Self::Null) => Ordering::Equal,
            (Self::Missing, _) => Ordering::Greater,
            (_, Self::Missing) => Ordering::Less,
            (Self::Null, _) => Ordering::Greater,
            (_, Self::Null) => Ordering::Less,
            (Self::Number(a), Self::Number(b)) => a.total_cmp(b),
            (Self::Text(a), Self::Text(b)) => a.encode_utf16().cmp(b.encode_utf16()),
            (Self::Bool(a), Self::Bool(b)) => a.cmp(b),
            _ => unreachable!("one song sort uses homogeneous keys"),
        }
    }
}
fn number(n: Option<f64>) -> Key {
    n.map(|n| Key::Number(if n == 0.0 { 0.0 } else { n }))
        .unwrap_or(Key::Null)
}
fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468
}
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (y + i64::from(m <= 2), m, d)
}
// Portable UTC interpretation of ISO created timestamps. Non-ISO/invalid dates
// fail rather than relying on host Date.parse coercions or NaN comparators.
// Reproduce source new Date(year,month,day)'s 0..99 -> 1900..1999 remapping.
fn created_day(value: &str) -> Result<i64, ExchangeError> {
    fn parse(s: &str) -> Option<i64> {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    }
    let invalid = ExchangeError::Malformed;
    if !value.is_ascii() {
        return Err(invalid);
    }
    let (date, time) = value
        .split_once(['T', 't', ' '])
        .map_or((value, None), |(d, t)| (d, Some(t)));
    let (prefix, rest) = if date.starts_with(['+', '-']) {
        if date.len() < 7 {
            return Err(invalid);
        }
        (&date[..7], &date[7..])
    } else {
        if date.len() < 4 {
            return Err(invalid);
        }
        (&date[..4], &date[4..])
    };
    let year = if let Some(p) = prefix.strip_prefix('-') {
        -parse(p).ok_or(invalid)?
    } else {
        parse(prefix.strip_prefix('+').unwrap_or(prefix)).ok_or(invalid)?
    };
    if prefix == "-000000" {
        return Err(invalid);
    }
    let (month, day) = match rest.len() {
        0 => (1, 1),
        3 if rest.starts_with('-') => (parse(&rest[1..]).ok_or(invalid)?, 1),
        6 if rest.as_bytes()[0] == b'-' && rest.as_bytes()[3] == b'-' => (
            parse(&rest[1..3]).ok_or(invalid)?,
            parse(&rest[4..]).ok_or(invalid)?,
        ),
        _ => return Err(invalid),
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(invalid);
    }
    let mut millis = days(year, month, day) * 86_400_000;
    if let Some(time) = time {
        // Datetimes require a complete date, unlike source-supported ISO dates.
        if rest.len() != 6 {
            return Err(invalid);
        }
        let (clock, offset) = if time.ends_with(['Z', 'z']) {
            (&time[..time.len() - 1], 0)
        } else if let Some(i) = time.find(['+', '-']) {
            let zone = &time[i + 1..];
            let (h, m) = match zone.len() {
                5 if zone.as_bytes()[2] == b':' => (parse(&zone[..2]), parse(&zone[3..])),
                4 => (parse(&zone[..2]), parse(&zone[2..])),
                _ => return Err(invalid),
            };
            let (h, m) = (h.ok_or(invalid)?, m.ok_or(invalid)?);
            if h > 23 || m > 59 {
                return Err(invalid);
            }
            (
                &time[..i],
                (h * 60 + m) * if time.as_bytes()[i] == b'+' { 1 } else { -1 },
            )
        } else {
            (time, 0)
        };
        let mut parts = clock.split(':');
        let hour = parts.next().ok_or(invalid)?;
        let minute = parts.next().ok_or(invalid)?;
        let second = parts.next();
        if parts.next().is_some() || hour.len() != 2 || minute.len() != 2 {
            return Err(invalid);
        }
        let h = parse(hour).ok_or(invalid)?;
        let m = parse(minute).ok_or(invalid)?;
        let (sec, fraction) = second.map_or(("00", None), |s| {
            s.split_once('.').map_or((s, None), |(s, f)| (s, Some(f)))
        });
        if sec.len() != 2 {
            return Err(invalid);
        }
        let sec = parse(sec).ok_or(invalid)?;
        let fraction = match fraction {
            Some(f) if !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()) => {
                let mut ms = 0;
                for i in 0..3 {
                    ms = ms * 10 + i64::from(f.as_bytes().get(i).copied().unwrap_or(b'0') - b'0');
                }
                ms
            }
            None => 0,
            _ => return Err(invalid),
        };
        if h > 24 || m > 59 || sec > 59 || (h == 24 && (m != 0 || sec != 0 || fraction != 0)) {
            return Err(invalid);
        }
        millis += (h * 3600 + m * 60 + sec) * 1000 + fraction - offset * 60_000;
    }
    if !(-8_640_000_000_000_000..=8_640_000_000_000_000).contains(&millis) {
        return Err(invalid);
    }
    let day = millis.div_euclid(86_400_000);
    let (y, m, d) = civil(day);
    Ok(if (0..=99).contains(&y) {
        days(y + 1900, m, d)
    } else {
        day
    })
}
pub(super) fn sorted(
    mut rows: Vec<Row>,
    sort: SongListSort,
    order: SortOrder,
    random: &mut dyn Random,
) -> Result<Vec<Row>, ExchangeError> {
    if sort == SongListSort::ExplicitStatus {
        return Ok(rows);
    }
    if sort == SongListSort::Id {
        if order == SortOrder::Desc {
            rows.reverse();
        }
        return Ok(rows);
    }
    if sort == SongListSort::Random {
        for i in 0..rows.len() {
            let unit = (random.next_u64() >> 11) as f64 / 9_007_199_254_740_992.0;
            let remaining = rows.len() - i;
            let offset = ((remaining as f64 * unit).floor() as usize).min(remaining - 1);
            rows.swap(i, i + offset);
        }
        return Ok(rows);
    }
    let mut keyed = Vec::new();
    for row in rows {
        let s = &row.song;
        let text = |v: &str| Key::Text(v.to_lowercase());
        let album = || text(s.album.as_deref().unwrap_or(""));
        let suffix = || {
            vec![
                album(),
                number(Some(s.disc_number as f64)),
                number(Some(s.track_number as f64)),
            ]
        };
        let mut keys = match sort {
            SongListSort::Album => vec![],
            SongListSort::AlbumArtist => vec![
                s.album_artists
                    .first()
                    .map(|a| text(&a.name))
                    .unwrap_or(Key::Missing),
            ],
            SongListSort::Artist => vec![text(&s.artist_name)],
            SongListSort::Bpm => vec![number(s.bpm)],
            SongListSort::Channels => vec![number(s.channels.map(|v| v as f64))],
            SongListSort::Comment => vec![Key::Null],
            SongListSort::Duration => vec![number(Some(row.duration_ms))],
            SongListSort::Favorited => vec![Key::Bool(s.user_favorite), text(&s.name)],
            SongListSort::Genre => vec![
                s.genres
                    .first()
                    .map(|g| text(&g.name))
                    .unwrap_or(Key::Missing),
            ],
            SongListSort::Name => vec![text(&s.name), album()],
            SongListSort::PlayCount => vec![number(Some(s.play_count as f64))],
            SongListSort::Rating => vec![number(s.user_rating), text(&s.name)],
            SongListSort::RecentlyAdded => vec![number(Some(created_day(&s.created_at)? as f64))],
            SongListSort::RecentlyPlayed => vec![Key::Null],
            SongListSort::ReleaseDate => vec![
                s.release_date
                    .as_ref()
                    .map(|v| Key::Text(v.clone()))
                    .unwrap_or(Key::Null),
            ],
            SongListSort::ReleaseYear => vec![number(s.release_year.map(f64::from))],
            SongListSort::SortName => vec![Key::Text(s.sort_name.clone())],
            SongListSort::Year => vec![number(s.year.map(f64::from))],
            _ => unreachable!("special sorts returned before scalar key construction"),
        };
        match sort {
            SongListSort::Name | SongListSort::SortName => {}
            SongListSort::Artist => keys.extend([
                number(Some(s.disc_number as f64)),
                number(Some(s.track_number as f64)),
            ]),
            _ => keys.extend(suffix()),
        }
        keyed.push((keys, row));
    }
    keyed.sort_by(|(a, _), (b, _)| {
        for (a, b) in a.iter().zip(b) {
            let c = a.compare(b);
            if c != Ordering::Equal {
                return if order == SortOrder::Desc {
                    c.reverse()
                } else {
                    c
                };
            }
        }
        Ordering::Equal
    });
    Ok(keyed.into_iter().map(|(_, row)| row).collect())
}
