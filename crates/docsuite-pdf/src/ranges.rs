use docsuite_core::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageRanges {
    segments: Vec<(u32, Option<u32>)>,
}

impl PageRanges {
    pub fn from_range(start: u32, end: Option<u32>) -> Self {
        Self {
            segments: vec![(start, end)],
        }
    }

    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        if trimmed.eq_ignore_ascii_case("all") {
            return Ok(Self {
                segments: vec![(1, None)],
            });
        }
        if trimmed.is_empty() {
            return Err(Error::InvalidInput("empty page range".to_owned()));
        }
        let mut segments = Vec::new();
        for part in trimmed.split(',') {
            let part = part.trim();
            if part.is_empty() {
                return Err(Error::InvalidInput(format!("empty segment in '{input}'")));
            }
            if let Some((start_str, end_str)) = part.split_once('-') {
                let start_str = start_str.trim();
                if start_str.is_empty() {
                    return Err(Error::InvalidInput(format!("invalid range '{part}'")));
                }
                let start: u32 = start_str
                    .parse()
                    .map_err(|_| Error::InvalidInput(format!("invalid page '{start_str}'")))?;
                if start == 0 {
                    return Err(Error::InvalidInput("page numbers are 1-based".to_owned()));
                }
                let end = if end_str.trim().is_empty() {
                    None
                } else {
                    let end: u32 = end_str
                        .trim()
                        .parse()
                        .map_err(|_| Error::InvalidInput(format!("invalid page '{end_str}'")))?;
                    if end == 0 {
                        return Err(Error::InvalidInput("page numbers are 1-based".to_owned()));
                    }
                    if end < start {
                        return Err(Error::InvalidInput(format!(
                            "invalid range '{part}': end before start"
                        )));
                    }
                    Some(end)
                };
                segments.push((start, end));
            } else {
                let page: u32 = part
                    .parse()
                    .map_err(|_| Error::InvalidInput(format!("invalid page '{part}'")))?;
                if page == 0 {
                    return Err(Error::InvalidInput("page numbers are 1-based".to_owned()));
                }
                segments.push((page, Some(page)));
            }
        }
        Ok(Self { segments })
    }

    pub fn pages(&self, page_count: u32) -> Result<Vec<u32>> {
        if page_count == 0 {
            return Err(Error::InvalidInput("document has no pages".to_owned()));
        }
        let mut pages = Vec::new();
        for (start, end) in &self.segments {
            let end = end.unwrap_or(page_count);
            if *start > page_count || end > page_count {
                return Err(Error::InvalidInput(format!(
                    "page {start}..{end} outside document ({page_count} pages)"
                )));
            }
            for p in *start..=end {
                if pages.last().copied() != Some(p) {
                    pages.push(p);
                }
            }
        }
        if pages.is_empty() {
            return Err(Error::InvalidInput("no pages in range".to_owned()));
        }
        Ok(pages)
    }

    pub fn single_pages(&self, page_count: u32) -> Result<Vec<Vec<u32>>> {
        let pages = self.pages(page_count)?;
        let mut groups = Vec::new();
        let mut current = Vec::new();
        let mut prev: Option<u32> = None;
        for p in pages {
            if prev.is_some_and(|v| p != v + 1) && !current.is_empty() {
                groups.push(current);
                current = Vec::new();
            }
            current.push(p);
            prev = Some(p);
        }
        if !current.is_empty() {
            groups.push(current);
        }
        Ok(groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_and_singles() {
        let all = PageRanges::parse("all").unwrap();
        assert_eq!(all.pages(3).unwrap(), vec![1, 2, 3]);
        let r = PageRanges::parse("1-3,5,7-").unwrap();
        assert_eq!(r.pages(10).unwrap(), vec![1, 2, 3, 5, 7, 8, 9, 10]);
    }

    #[test]
    fn parse_rejects_invalid() {
        assert!(PageRanges::parse("0").is_err());
        assert!(PageRanges::parse("3-1").is_err());
        assert!(PageRanges::parse("99").unwrap().pages(5).is_err());
    }
}
