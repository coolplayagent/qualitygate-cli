//! Strict bounded subset of EDN emitted by the audited metric tools.

use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub(crate) enum Value {
    Map(BTreeMap<String, Value>),
    List(Vec<Value>),
    String(String),
    Number(String),
    Nil,
}

impl Value {
    pub fn map(self) -> Result<BTreeMap<String, Self>> {
        match self {
            Self::Map(v) => Ok(v),
            _ => bail!("Expected EDN map"),
        }
    }
    pub fn list(self) -> Result<Vec<Self>> {
        match self {
            Self::List(v) => Ok(v),
            _ => bail!("Expected EDN vector"),
        }
    }
    pub fn string(self) -> Result<String> {
        match self {
            Self::String(v) => Ok(v),
            _ => bail!("Expected EDN string"),
        }
    }
    pub fn number(self) -> Result<String> {
        match self {
            Self::Number(v) => Ok(v),
            _ => bail!("Expected finite EDN numeric token"),
        }
    }
}

pub(crate) fn take(map: &mut BTreeMap<String, Value>, key: &str) -> Result<Value> {
    map.remove(key)
        .with_context(|| format!("Missing required EDN field: {key}"))
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Value> {
    ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        "Raw tool report exceeds 16 MiB"
    );
    std::str::from_utf8(bytes).context("EDN report is not UTF-8")?;
    let mut parser = Parser {
        bytes,
        cursor: 0,
        nodes: 0,
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let value = parser.value(0)?;
    parser.space();
    ensure!(parser.cursor == bytes.len(), "Trailing EDN input");
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    cursor: usize,
    nodes: usize,
    deadline: Instant,
}

impl Parser<'_> {
    fn space(&mut self) {
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(|v| v.is_ascii_whitespace() || *v == b',')
        {
            self.cursor += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value> {
        self.space();
        ensure!(
            depth <= 16 && self.nodes < 500_000 && Instant::now() < self.deadline,
            "EDN work budget exceeded"
        );
        self.nodes += 1;
        match self
            .bytes
            .get(self.cursor)
            .copied()
            .context("Truncated EDN value")?
        {
            b'{' => self.map(depth),
            b'[' => self.vector(depth),
            b'"' => Ok(Value::String(self.string()?)),
            b'-' | b'0'..=b'9' => {
                let token = self.token()?;
                super::decimal::Decimal::parse(&token)?;
                Ok(Value::Number(token))
            }
            b'n' => {
                ensure!(self.token()? == "nil", "Unsupported EDN symbol");
                Ok(Value::Nil)
            }
            _ => bail!("Unsupported EDN token (tags, symbols and executable forms are forbidden)"),
        }
    }

    fn token(&mut self) -> Result<String> {
        let start = self.cursor;
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(|v| !v.is_ascii_whitespace() && !b",]}".contains(v))
        {
            self.cursor += 1;
        }
        ensure!(self.cursor - start <= 4096, "EDN token exceeds bound");
        Ok(std::str::from_utf8(&self.bytes[start..self.cursor])?.to_string())
    }

    fn map(&mut self, depth: usize) -> Result<Value> {
        self.cursor += 1;
        let mut map = BTreeMap::new();
        loop {
            self.space();
            if self.bytes.get(self.cursor) == Some(&b'}') {
                self.cursor += 1;
                return Ok(Value::Map(map));
            }
            ensure!(
                self.bytes.get(self.cursor) == Some(&b':'),
                "EDN map requires keyword keys"
            );
            self.cursor += 1;
            let key = self.token()?;
            ensure!(
                !key.is_empty()
                    && key
                        .bytes()
                        .all(|v| v.is_ascii_alphanumeric() || b"_-".contains(&v)),
                "Invalid EDN key"
            );
            let value = self.value(depth + 1)?;
            ensure!(map.insert(key, value).is_none(), "Repeated EDN key");
        }
    }

    fn vector(&mut self, depth: usize) -> Result<Value> {
        self.cursor += 1;
        let mut list = Vec::new();
        loop {
            self.space();
            if self.bytes.get(self.cursor) == Some(&b']') {
                self.cursor += 1;
                return Ok(Value::List(list));
            }
            ensure!(list.len() < 50_000, "EDN vector exceeds 50,000 records");
            list.push(self.value(depth + 1)?);
        }
    }

    fn string(&mut self) -> Result<String> {
        let start = self.cursor;
        self.cursor += 1;
        let mut escaped = false;
        loop {
            let byte = *self
                .bytes
                .get(self.cursor)
                .context("Truncated EDN string")?;
            self.cursor += 1;
            ensure!(self.cursor - start <= 16 * 1024, "EDN string exceeds bound");
            if byte == b'"' && !escaped {
                break;
            }
            escaped = byte == b'\\' && !escaped;
        }
        // The audited writers emit the JSON-compatible EDN string subset.
        serde_json::from_slice(&self.bytes[start..self.cursor])
            .context("Unsupported EDN string escape")
    }
}

#[cfg(test)]
#[path = "edn_tests.rs"]
mod tests;
