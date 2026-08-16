use std::fs;
use std::path::Path;

#[path = "lcov/function.rs"]
mod function;
#[path = "lcov/value.rs"]
mod value;

use crate::model::{Coverage, FileCoverage, Manifest};
use crate::path;
use function::FunctionRecords;
use value::{branch_hits, fields, hits, insert_hit, number};

pub fn parse_many(
    inputs: &[impl AsRef<Path>],
    workspace: &Path,
    manifest: &Manifest,
) -> Result<Coverage, String> {
    if inputs.is_empty() {
        return Err("no LCOV inputs provided".into());
    }
    let mut combined = Coverage::default();
    for input in inputs {
        let input = input.as_ref();
        let coverage = parse(input, workspace, manifest)
            .map_err(|error| format!("{}: {error}", input.display()))?;
        // Validate each shard before merging so an ordinary LCOV cannot borrow BRDA
        // provenance from a different input.
        if !coverage
            .files
            .values()
            .any(|file| !file.branches.is_empty())
        {
            return Err(format!(
                "{}: branch LCOV contains no scoped BRDA records",
                input.display()
            ));
        }
        combined.merge(coverage);
    }
    Ok(combined)
}

pub fn parse(input: &Path, workspace: &Path, manifest: &Manifest) -> Result<Coverage, String> {
    let text = fs::read_to_string(input)
        .map_err(|error| format!("cannot read LCOV input {}: {error}", input.display()))?;
    let mut coverage = Coverage::default();
    let mut record: Option<Record> = None;
    for (index, raw) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw.trim_end_matches('\r');
        if let Some(sf) = line.strip_prefix("SF:") {
            if record.is_some() {
                return Err(format!("line {line_no}: SF before end_of_record"));
            }
            record = Some(Record::new(path::normalize(sf, workspace, manifest)));
        } else if line == "end_of_record" {
            let item = record
                .take()
                .ok_or_else(|| format!("line {line_no}: end_of_record without SF"))?;
            item.finish(&mut coverage)?;
        } else if let Some(item) = record.as_mut() {
            item.consume(line, line_no)?;
        } else if relevant_outside_record(line) {
            return Err(format!("line {line_no}: relevant record appears before SF"));
        }
    }
    if record.is_some() {
        return Err("LCOV input ends before end_of_record".into());
    }
    if coverage.files.is_empty() {
        return Err("LCOV contains no scoped production records".into());
    }
    Ok(coverage)
}

struct Record {
    path: Option<String>,
    data: FileCoverage,
    functions: FunctionRecords,
}

impl Record {
    fn new(path: Option<String>) -> Self {
        Self {
            path,
            data: FileCoverage::default(),
            functions: FunctionRecords::default(),
        }
    }

    fn consume(&mut self, line: &str, line_no: usize) -> Result<(), String> {
        if self.path.is_none() {
            return Ok(());
        }
        if let Some(value) = line.strip_prefix("DA:") {
            let fields = fields(value, 2, 3, line_no, "DA")?;
            insert_hit(
                &mut self.data.lines,
                number(fields[0], line_no)?,
                hits(fields[1], line_no)?,
                line_no,
                "DA",
            )?;
        } else if let Some(value) = line.strip_prefix("FN:") {
            self.functions.legacy_line(value, line_no, &mut self.data)?;
        } else if let Some(value) = line.strip_prefix("FNDA:") {
            self.functions.legacy_hits(value, line_no, &mut self.data)?;
        } else if let Some(value) = line.strip_prefix("FNL:") {
            self.functions.indexed_line(value, line_no)?;
        } else if let Some(value) = line.strip_prefix("FNA:") {
            self.functions.indexed_hit(value, line_no)?;
        } else if let Some(value) = line.strip_prefix("BRDA:") {
            let f = fields(value, 4, 4, line_no, "BRDA")?;
            let key = (
                number(f[0], line_no)?,
                number(f[1], line_no)?,
                number(f[2], line_no)?,
            );
            if self
                .data
                .branches
                .insert(key, branch_hits(f[3], line_no)?)
                .is_some()
            {
                return Err(format!("line {line_no}: duplicate BRDA key in one record"));
            }
        } else if let Some(value) = line.strip_prefix("RG:") {
            let f = fields(value, 5, 5, line_no, "RG")?;
            let key = (
                number(f[0], line_no)?,
                number(f[1], line_no)?,
                number(f[2], line_no)?,
                number(f[3], line_no)?,
            );
            insert_hit(
                &mut self.data.regions,
                key,
                hits(f[4], line_no)?,
                line_no,
                "RG",
            )?;
            self.data.explicit_regions = true;
        } else if is_relevant(line) && !is_summary(line) {
            return Err(format!(
                "line {line_no}: malformed relevant LCOV record: {line}"
            ));
        }
        Ok(())
    }

    fn finish(mut self, coverage: &mut Coverage) -> Result<(), String> {
        let Some(path) = self.path else { return Ok(()) };
        self.functions.finish(&path, &mut self.data)?;
        coverage.files.entry(path).or_default().merge(self.data);
        Ok(())
    }
}

fn relevant_outside_record(line: &str) -> bool {
    is_relevant(line) || line == "end_of_record"
}
fn is_relevant(line: &str) -> bool {
    ["DA:", "FN:", "FNDA:", "FNL:", "FNA:", "BRDA:", "RG:"]
        .iter()
        .any(|p| line.starts_with(p))
}
fn is_summary(line: &str) -> bool {
    ["FNF:", "FNH:", "BRF:", "BRH:", "LF:", "LH:"]
        .iter()
        .any(|p| line.starts_with(p))
}
