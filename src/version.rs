//! Ports of `System.Version` and `Microsoft.VisualStudio.Setup.VersionRange`.

use std::cmp::Ordering;
use std::fmt;

/// `System.Version`: 2-4 numeric components; build/revision are -1 when absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Version {
    pub major: i32,
    pub minor: i32,
    pub build: i32,
    pub revision: i32,
}

impl Version {
    pub fn new(major: i32, minor: i32, build: i32, revision: i32) -> Self {
        Self {
            major,
            minor,
            build,
            revision,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.trim().split('.').collect();
        if !(2..=4).contains(&parts.len()) {
            return None;
        }
        let mut n = [-1i32; 4];
        for (i, p) in parts.iter().enumerate() {
            let p = p.trim();
            if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit() || c == '+') {
                return None;
            }
            n[i] = p.parse::<i32>().ok().filter(|v| *v >= 0)?;
        }
        Some(Self::new(n[0], n[1], n[2], n[3]))
    }
}

impl Ord for Version {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.major, self.minor, self.build, self.revision)
            .cmp(&(o.major, o.minor, o.build, o.revision))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)?;
        if self.build >= 0 {
            write!(f, ".{}", self.build)?;
            if self.revision >= 0 {
                write!(f, ".{}", self.revision)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionRange {
    pub minimum: Option<Version>,
    pub maximum: Option<Version>,
    pub is_minimum_inclusive: bool,
    pub is_maximum_inclusive: bool,
}

impl VersionRange {
    pub fn new(min: Option<Version>, min_incl: bool, max: Option<Version>, max_incl: bool) -> Self {
        Self {
            minimum: min,
            maximum: max,
            is_minimum_inclusive: min.is_some() && min_incl,
            is_maximum_inclusive: max.is_some() && max_incl,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let v = s.trim();
        if v.is_empty() {
            return Some(Self::new(None, true, None, true));
        }
        let min_incl = match v.as_bytes()[0] {
            b'(' => false,
            b'[' => true,
            _ => {
                let ver = Version::parse(v)?;
                return Some(Self::new(Some(ver), true, None, false));
            }
        };
        if v.len() < 2 {
            return None;
        }
        let max_incl = match v.as_bytes()[v.len() - 1] {
            b')' => false,
            b']' => true,
            _ => return None,
        };
        let inner = &v[1..v.len() - 1];
        let (lo, hi) = match inner.find(',') {
            Some(i) => (inner[..i].trim(), inner[i + 1..].trim()),
            None => (inner.trim(), inner.trim()),
        };
        let get = |x: &str| -> Result<Option<Version>, ()> {
            if x.is_empty() {
                Ok(None)
            } else {
                Version::parse(x).map(Some).ok_or(())
            }
        };
        let (lo, hi) = (get(lo).ok()?, get(hi).ok()?);
        Some(Self::new(lo, min_incl, hi, max_incl))
    }
}

fn version_string(v: Option<Version>, num: i32) -> String {
    match v {
        None => String::new(),
        Some(v) if v.build == num && v.revision == num => format!("{}.{}", v.major, v.minor),
        Some(v) => v.to_string(),
    }
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lo = if self.maximum.is_none() {
            String::new()
        } else {
            version_string(self.minimum, 0)
        };
        let hi = if self.maximum.is_none() {
            String::new()
        } else {
            version_string(self.maximum, i32::MAX)
        };
        if let (Some(min), None, true) = (self.minimum, self.maximum, self.is_minimum_inclusive) {
            return write!(f, "{min}");
        }
        if self.minimum == self.maximum {
            return match self.minimum {
                Some(_) => write!(f, "[{lo}]"),
                None => Ok(()),
            };
        }
        write!(
            f,
            "{}{},{}{}",
            if self.is_minimum_inclusive { "[" } else { "(" },
            lo,
            hi,
            if self.is_maximum_inclusive { "]" } else { ")" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parse_and_display() {
        assert_eq!(Version::parse("1.2").unwrap().to_string(), "1.2");
        assert_eq!(Version::parse(" 1.2.3.4 ").unwrap().to_string(), "1.2.3.4");
        assert_eq!(Version::parse("17.0.1").unwrap().build, 1);
        assert!(Version::parse("1").is_none());
        assert!(Version::parse("1.2.3.4.5").is_none());
        assert!(Version::parse("1.x").is_none());
        assert!(Version::parse("1.-1").is_none());
    }

    #[test]
    fn version_ordering() {
        assert!(Version::parse("1.2").unwrap() < Version::parse("1.2.0").unwrap());
        assert!(Version::parse("17.0").unwrap() < Version::parse("18.0").unwrap());
    }

    fn rt(s: &str) -> String {
        VersionRange::parse(s).unwrap().to_string()
    }

    #[test]
    fn range_round_trips() {
        assert_eq!(rt("[17.0,18.0)"), "[17.0,18.0)");
        assert_eq!(rt("[17.0, 18.0]"), "[17.0,18.0]");
        // .NET quirk: open-ended ranges lose their bound when printed
        assert_eq!(rt("(15.0,)"), "(,)");
        assert_eq!(rt("[15.0,)"), "15.0");
        assert_eq!(rt("17.0"), "17.0");
        assert_eq!(rt("[0.0,0.0]"), "[0.0]");
        assert_eq!(rt("[16.0]"), "[16.0]");
        assert_eq!(rt(""), "");
        assert_eq!(rt("[17.0.1,18.0)"), "[17.0.1,18.0)");
    }

    #[test]
    fn range_rejects_invalid() {
        assert!(VersionRange::parse("[17.0,18.0").is_none());
        assert!(VersionRange::parse("[a,b]").is_none());
        assert!(VersionRange::parse("abc").is_none());
    }

    #[test]
    fn range_fields() {
        let r = VersionRange::parse("[17.0,18.0)").unwrap();
        assert!(r.is_minimum_inclusive);
        assert!(!r.is_maximum_inclusive);
        assert_eq!(r.maximum, Version::parse("18.0"));
    }
}
