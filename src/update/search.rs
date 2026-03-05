use crate::app::{App, FinderMode, FinderResult};
use crate::model::mode::Mode;
use anyhow::Result;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

impl App {
    pub(crate) fn open_finder(&mut self, mode: FinderMode) -> Result<()> {
        self.mode = Mode::FinderOpen;
        self.finder_mode = mode;
        self.finder_query.clear();
        self.finder_selected = 0;
        self.file_tree.refresh()?;
        self.refresh_finder_results()
    }
    pub(crate) fn refresh_finder_results(&mut self) -> Result<()> {
        self.file_tree.refresh()?;

        let files = self.file_tree.all_file_paths();
        let limit = self.config.search.max_results;

        self.finder_results.clear();

        if self.finder_mode == FinderMode::Files {
            if self.finder_query.is_empty() {
                self.finder_results = files
                    .into_iter()
                    .take(limit)
                    .map(|path| FinderResult {
                        preview: path.to_string_lossy().to_string(),
                        path,
                        line: None,
                    })
                    .collect();
                self.finder_selected = 0;
                return Ok(());
            }

            let matcher = SkimMatcherV2::default();
            let mut scored: Vec<(i64, FinderResult)> = files
                .into_iter()
                .filter_map(|path| {
                    let candidate = path.to_string_lossy().to_string();
                    matcher
                        .fuzzy_match(&candidate, &self.finder_query)
                        .map(|score| {
                            (
                                score,
                                FinderResult {
                                    path,
                                    line: None,
                                    preview: candidate,
                                },
                            )
                        })
                })
                .collect();

            scored.sort_by(|a, b| b.0.cmp(&a.0));

            self.finder_results = scored
                .into_iter()
                .take(limit)
                .map(|(_, item)| item)
                .collect();
        } else {
            if self.finder_query.is_empty() {
                self.finder_selected = 0;
                return Ok(());
            }

            let needle = self.finder_query.to_lowercase();
            let mut hits = Vec::new();

            for path in files {
                let Ok(contents) = std::fs::read_to_string(&path) else {
                    continue;
                };

                for (idx, line) in contents.lines().enumerate() {
                    if line.to_lowercase().contains(&needle) {
                        hits.push(FinderResult {
                            preview: format!(
                                "{}:{}  {}",
                                path.to_string_lossy(),
                                idx + 1,
                                line.trim()
                            ),
                            path: path.clone(),
                            line: Some(idx + 1),
                        });
                        if hits.len() >= limit {
                            break;
                        }
                    }
                }

                if hits.len() >= limit {
                    break;
                }
            }

            self.finder_results = hits;
        }

        if self.finder_results.is_empty() {
            self.finder_selected = 0;
        } else if self.finder_selected >= self.finder_results.len() {
            self.finder_selected = self.finder_results.len() - 1;
        }

        Ok(())
    }
}
