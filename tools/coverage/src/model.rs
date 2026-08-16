use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Counter {
    pub found: u64,
    pub hit: u64,
}

impl Counter {
    pub fn percent(&self) -> f64 {
        if self.found == 0 {
            0.0
        } else {
            100.0 * self.hit as f64 / self.found as f64
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FileCoverage {
    pub lines: BTreeMap<u32, u64>,
    pub functions: BTreeMap<String, u64>,
    pub function_lines: BTreeMap<String, u32>,
    pub branches: BTreeMap<(u32, u32, u32), Option<u64>>,
    pub regions: BTreeMap<(u32, u32, u32, u32), u64>,
    pub explicit_regions: bool,
}

impl FileCoverage {
    pub fn merge(&mut self, other: Self) {
        merge_hits(&mut self.lines, other.lines);
        merge_hits(&mut self.functions, other.functions);
        for (name, line) in other.function_lines {
            self.function_lines.entry(name).or_insert(line);
        }
        for (key, hits) in other.branches {
            let slot = self.branches.entry(key).or_insert(Some(0));
            *slot = match (*slot, hits) {
                (Some(a), Some(b)) => Some(a.saturating_add(b)),
                (a @ Some(_), None) => a,
                (None, b @ Some(_)) => b,
                (None, None) => None,
            };
        }
        merge_hits(&mut self.regions, other.regions);
        self.explicit_regions |= other.explicit_regions;
    }

    pub fn line_counter(&self) -> Counter {
        counter(self.lines.values().copied())
    }

    pub fn function_counter(&self) -> Counter {
        let mut by_source_line = BTreeMap::new();
        for (name, hits) in &self.functions {
            if is_source_function(name)
                && let Some(line) = self.function_lines.get(name)
            {
                let slot = by_source_line.entry(*line).or_insert(0u64);
                *slot = slot.saturating_add(*hits);
            }
        }
        counter(by_source_line.into_values())
    }

    pub fn branch_counter(&self) -> Counter {
        counter(self.branches.values().map(|hits| hits.unwrap_or(0)))
    }

    pub fn explicit_region_counter(&self) -> Counter {
        counter(self.regions.values().copied())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriticalFunction {
    pub path: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub sources: BTreeSet<String>,
    pub critical: Vec<CriticalFunction>,
}

#[derive(Clone, Debug, Default)]
pub struct Coverage {
    pub files: BTreeMap<String, FileCoverage>,
}

impl Coverage {
    pub fn merge(&mut self, other: Self) {
        for (path, file) in other.files {
            self.files.entry(path).or_default().merge(file);
        }
    }
}

fn is_source_function(symbol: &str) -> bool {
    !symbol.starts_with("_RNC")
}

fn counter(hits: impl Iterator<Item = u64>) -> Counter {
    hits.fold(Counter::default(), |mut result, value| {
        result.found += 1;
        result.hit += u64::from(value > 0);
        result
    })
}

fn merge_hits<K: Ord>(target: &mut BTreeMap<K, u64>, source: BTreeMap<K, u64>) {
    for (key, hits) in source {
        let slot = target.entry(key).or_default();
        *slot = slot.saturating_add(hits);
    }
}
