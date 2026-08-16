use std::collections::{BTreeMap, BTreeSet};

use crate::model::FileCoverage;

use super::value::{hits, insert_hit, number};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Legacy,
    Indexed,
}

#[derive(Default)]
pub(super) struct FunctionRecords {
    format: Option<Format>,
    legacy_names: BTreeSet<String>,
    indexed_lines: BTreeMap<u32, u32>,
    indexed_hits: BTreeMap<u32, (String, u64)>,
}

impl FunctionRecords {
    pub(super) fn legacy_line(
        &mut self,
        value: &str,
        line_no: usize,
        data: &mut FileCoverage,
    ) -> Result<(), String> {
        self.select_format(Format::Legacy, line_no)?;
        let (start, name) = value
            .split_once(',')
            .ok_or_else(|| format!("line {line_no}: malformed FN record"))?;
        if name.is_empty() {
            return Err(format!("line {line_no}: empty FN name"));
        }
        if !self.legacy_names.insert(name.into()) {
            return Err(format!("line {line_no}: duplicate FN name in one record"));
        }
        data.function_lines
            .insert(name.into(), number(start, line_no)?);
        Ok(())
    }

    pub(super) fn legacy_hits(
        &mut self,
        value: &str,
        line_no: usize,
        data: &mut FileCoverage,
    ) -> Result<(), String> {
        self.select_format(Format::Legacy, line_no)?;
        let (count, name) = value
            .split_once(',')
            .ok_or_else(|| format!("line {line_no}: malformed FNDA record"))?;
        if name.is_empty() {
            return Err(format!("line {line_no}: empty FNDA name"));
        }
        insert_hit(
            &mut data.functions,
            name.into(),
            hits(count, line_no)?,
            line_no,
            "FNDA",
        )
    }

    pub(super) fn indexed_line(&mut self, value: &str, line_no: usize) -> Result<(), String> {
        self.select_format(Format::Indexed, line_no)?;
        let (index, start) = pair(value, line_no, "FNL")?;
        if self.indexed_lines.insert(index, start).is_some() {
            return Err(format!("line {line_no}: duplicate FNL index"));
        }
        Ok(())
    }

    pub(super) fn indexed_hit(&mut self, value: &str, line_no: usize) -> Result<(), String> {
        self.select_format(Format::Indexed, line_no)?;
        let mut fields = value.splitn(3, ',');
        let index = number(required(fields.next(), line_no, "FNA")?, line_no)?;
        let count = hits(required(fields.next(), line_no, "FNA")?, line_no)?;
        let name = required(fields.next(), line_no, "FNA")?;
        if self
            .indexed_hits
            .insert(index, (name.into(), count))
            .is_some()
        {
            return Err(format!("line {line_no}: duplicate FNA index"));
        }
        Ok(())
    }

    pub(super) fn finish(mut self, path: &str, data: &mut FileCoverage) -> Result<(), String> {
        for name in &self.legacy_names {
            if !data.functions.contains_key(name) {
                return Err(format!("{path}: FN has no matching function entry"));
            }
        }
        for name in data.functions.keys() {
            if !self.legacy_names.contains(name) {
                return Err(format!("{path}: FNDA has no matching FN entry: {name}"));
            }
        }
        if self.indexed_lines.keys().ne(self.indexed_hits.keys()) {
            return Err(format!("{path}: FNL/FNA function indices do not match"));
        }
        for (index, (name, count)) in self.indexed_hits {
            let line = self.indexed_lines.remove(&index).expect("matched index");
            if data.function_lines.insert(name.clone(), line).is_some()
                || data.functions.insert(name, count).is_some()
            {
                return Err(format!("{path}: duplicate function identity"));
            }
        }
        Ok(())
    }

    fn select_format(&mut self, format: Format, line_no: usize) -> Result<(), String> {
        if self.format.is_some_and(|current| current != format) {
            return Err(format!(
                "line {line_no}: mixed legacy and indexed function records"
            ));
        }
        self.format = Some(format);
        Ok(())
    }
}

fn pair(value: &str, line_no: usize, kind: &str) -> Result<(u32, u32), String> {
    let (left, right) = value
        .split_once(',')
        .ok_or_else(|| format!("line {line_no}: malformed {kind} record"))?;
    Ok((number(left, line_no)?, number(right, line_no)?))
}

fn required<'a>(value: Option<&'a str>, line_no: usize, kind: &str) -> Result<&'a str, String> {
    value
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("line {line_no}: malformed {kind} record"))
}
