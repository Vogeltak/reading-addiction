//! Helper utilities to read Pocket exports for seeding our crawler.

use std::{fmt, io::Read};

use anyhow::Error;
use reqwest::Url;
use serde::Deserialize;

/// Reader for Pocket CSV export files.
pub struct PocketReader<R> {
    reader: R,
}

impl<R> PocketReader<R>
where
    R: Read,
{
    /// Creates a new [`PocketReader`].
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Processes all CSV rows into [`PocketItem`]s.
    pub fn read(self) -> Result<Vec<PocketItem>, Error> {
        let mut reader = csv::Reader::from_reader(self.reader);
        let mut items: Vec<PocketItem> = vec![];

        for item in reader.deserialize() {
            items.push(item?);
        }

        Ok(items)
    }
}

#[derive(Debug, Deserialize)]
pub struct PocketItem {
    pub title: String,
    pub url: Url,
    pub time_added: usize,
    pub tags: PocketTags,
    pub status: PocketStatus,
}

#[derive(Debug)]
pub struct PocketTags(Vec<Tag>);

impl<'de> Deserialize<'de> for PocketTags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        if s.is_empty() {
            return Ok(PocketTags(vec![]));
        }

        let tags = s.split('|').map(|tag| Tag(tag.to_string())).collect();

        Ok(PocketTags(tags))
    }
}

impl fmt::Display for PocketTags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let joined = self
            .0
            .iter()
            .map(|tag| tag.0.as_str())
            .collect::<Vec<&str>>()
            .join(",");

        write!(f, "{joined}")
    }
}

impl IntoIterator for PocketTags {
    type Item = Tag;

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, Deserialize)]
pub struct Tag(pub String);

#[derive(Debug, Clone, Deserialize)]
pub enum PocketStatus {
    #[serde(rename = "unread")]
    Unread,
    #[serde(rename = "archive")]
    Archive,
}

impl fmt::Display for PocketStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let out = match self {
            PocketStatus::Unread => "unread",
            PocketStatus::Archive => "archive",
        };

        write!(f, "{out}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trivial() {
        let data = "\
title,url,time_added,tags,status
What Do You Want to Do With Your Life? | Scott H Young,https://www.scotthyoung.com/blog/2007/07/29/what-do-you-want-to-do-with-your-life/,1592774907,,archive
Taoism,https://en.wikipedia.org/wiki/Taoism,1614076299,meaning,unread
https://www.yudkowsky.net/rational/virtues,https://www.yudkowsky.net/rational/virtues,1642196007,rationality|self improvement,unread";

        let pr = PocketReader::new(data.as_bytes());
        let items = pr.read().expect("should parse pocket items correctly");
        println!("{items:#?}");
    }
}
