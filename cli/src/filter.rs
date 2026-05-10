use chrono::{DateTime, Utc};

#[derive(Debug, Default, PartialEq)]
pub struct Filter {
    pub tags_include: Vec<String>,
    pub tags_exclude: Vec<String>,
    pub untagged: bool,
    pub domain: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub older_than: Option<DateTime<Utc>>,
    pub starred: Option<bool>,
    pub archived: Option<bool>,
    pub read: Option<bool>,
    pub search: Option<String>,
}

pub fn parse(input: &str) -> anyhow::Result<Filter> {
    let mut f = Filter::default();
    for raw in input.split(',') {
        let part = raw.trim();
        if part.is_empty() {
            continue;
        }
        let (neg, p) = if let Some(r) = part.strip_prefix('!') {
            (true, r)
        } else {
            (false, part)
        };

        if let Some(v) = p.strip_prefix("tag:") {
            if neg {
                f.tags_exclude.push(v.into());
            } else {
                f.tags_include.push(v.into());
            }
        } else if p == "untagged" && !neg {
            f.untagged = true;
        } else if let Some(v) = p.strip_prefix("domain:") {
            f.domain = Some(v.into());
        } else if let Some(v) = p.strip_prefix("since:") {
            f.since = Some(parse_time_point(v)?);
        } else if let Some(v) = p.strip_prefix("older-than:") {
            f.older_than = Some(parse_time_point(v)?);
        } else if p == "starred" {
            f.starred = Some(!neg);
        } else if p == "archived" {
            f.archived = Some(!neg);
        } else if p == "unread" && !neg {
            f.read = Some(false);
        } else if p == "read" && !neg {
            f.read = Some(true);
        } else if let Some(v) = p.strip_prefix("search:") {
            f.search = Some(v.into());
        } else {
            anyhow::bail!("unknown filter key: {part}");
        }
    }
    Ok(f)
}

fn parse_time_point(s: &str) -> anyhow::Result<DateTime<Utc>> {
    // Relative form: ^[0-9]+[hdw]$
    if !s.is_empty() {
        let last = s.chars().last().unwrap();
        if matches!(last, 'h' | 'd' | 'w') {
            let num_str = &s[..s.len() - 1];
            if let Ok(n) = num_str.parse::<i64>() {
                let dur = match last {
                    'h' => chrono::Duration::hours(n),
                    'd' => chrono::Duration::days(n),
                    'w' => chrono::Duration::weeks(n),
                    _ => unreachable!(),
                };
                return Ok(Utc::now() - dur);
            }
        }
    }
    // Absolute ISO date
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d.and_hms_opt(0, 0, 0).unwrap().and_utc());
    }
    // Absolute RFC3339
    Ok(DateTime::parse_from_rfc3339(s)?.with_timezone(&Utc))
}

impl Filter {
    /// Convert to HTTP query params for /api/v1/entries.
    pub fn to_query(&self) -> Vec<(&'static str, String)> {
        let mut q = vec![];
        if !self.tags_include.is_empty() {
            q.push(("tag", self.tags_include.join(",")));
        }
        if !self.tags_exclude.is_empty() {
            q.push(("exclude_tag", self.tags_exclude.join(",")));
        }
        if self.untagged {
            q.push(("untagged", "true".into()));
        }
        if let Some(d) = &self.domain {
            q.push(("domain", d.clone()));
        }
        if let Some(t) = self.since {
            q.push(("since", t.to_rfc3339()));
        }
        if let Some(t) = self.older_than {
            q.push(("before", t.to_rfc3339()));
        }
        if let Some(b) = self.starred {
            q.push(("is_starred", b.to_string()));
        }
        if let Some(b) = self.archived {
            q.push(("is_archived", b.to_string()));
        }
        if let Some(b) = self.read {
            q.push(("is_read", b.to_string()));
        }
        if let Some(s) = &self.search {
            q.push(("search", s.clone()));
        }
        q
    }
}
