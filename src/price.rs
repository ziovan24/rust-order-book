use std::fmt;
use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct Price(pub f64);

impl Price {
    pub fn as_f64(&self) -> f64 {
        self.0
    }
}

impl PartialEq for Price {
    fn eq(&self, other: &Self) -> bool {
        if self.0.is_nan() && other.0.is_nan() {
            true
        } else {
            self.0 == other.0
        }
    }
}

impl Eq for Price {}

impl PartialOrd for Price {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.0.is_nan() && other.0.is_nan() {
            Some(Ordering::Equal)
        } else if self.0.is_nan() {
            Some(Ordering::Less)
        } else if other.0.is_nan() {
            Some(Ordering::Greater)
        } else {
            self.0.partial_cmp(&other.0)
        }
    }
}

impl Ord for Price {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

