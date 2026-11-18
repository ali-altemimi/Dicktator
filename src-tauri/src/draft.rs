//! The draft: a plain text file the dictated text is appended to.
//!
//! It's an ordinary file (Documents/Dictator/draft.txt by default), so it can be
//! opened in any editor. Copying out moves the text to the clipboard and into
//! archive.txt, then empties the draft.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

pub struct Draft {
    path: PathBuf,
    archive: PathBuf,
    /// Byte ranges (start, end) of each chunk we appended, newest last, for undo.
    chunks: Vec<(u64, u64)>,
}

impl Draft {
    pub fn open(dir: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&dir)?;
        let path = dir.join("draft.txt");
        OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self { path, archive: dir.join("archive.txt"), chunks: Vec::new() })
    }

    pub fn text(&self) -> io::Result<String> {
        fs::read_to_string(&self.path)
    }

    /// Appends a chunk, adding a space between it and the previous text if needed.
    pub fn append(&mut self, text: &str) -> io::Result<()> {
        let mut f = OpenOptions::new().read(true).write(true).open(&self.path)?;
        let end = f.seek(SeekFrom::End(0))?;
        let mut last = [0u8; 1];
        if end > 0 {
            f.seek(SeekFrom::Start(end - 1))?;
            f.read_exact(&mut last)?;
        }
        let needs_space =
            end > 0 && !matches!(last[0], b' ' | b'\n') && !text.starts_with([' ', '\n']);
        let chunk = if needs_space { format!(" {text}") } else { text.to_owned() };
        f.seek(SeekFrom::End(0))?;
        f.write_all(chunk.as_bytes())?;
        self.chunks.push((end, end + chunk.len() as u64));
        Ok(())
    }

    /// Removes the last appended chunk. Returns false if there's nothing to undo,
    /// including when the file was edited or cleared since our last write.
    pub fn undo(&mut self) -> io::Result<bool> {
        let size = fs::metadata(&self.path)?.len();
        match self.chunks.last() {
            Some(&(start, end)) if end == size => {
                self.chunks.pop();
                File::options().write(true).open(&self.path)?.set_len(start)?;
                Ok(true)
            }
            _ => {
                self.chunks.clear();
                Ok(false)
            }
        }
    }

    /// Empties the draft, saving its text to the archive first.
    /// Returns the text, or None if the draft was empty.
    pub fn take(&mut self) -> io::Result<Option<String>> {
        let text = self.text()?;
        if text.trim().is_empty() {
            return Ok(None);
        }
        let mut archive = OpenOptions::new().create(true).append(true).open(&self.archive)?;
        write!(archive, "\n\n===== {} =====\n{text}", timestamp())?;
        File::options().write(true).open(&self.path)?.set_len(0)?;
        self.chunks.clear();
        Ok(Some(text))
    }
}

/// Local time as "YYYY-MM-DD HH:MM", without pulling in a date crate.
fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // UTC; good enough to tell archive entries apart.
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", rem / 3600, rem % 3600 / 60)
}

// Howard Hinnant's days-to-civil-date algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> (tempfile::TempDir, Draft) {
        let dir = tempfile::tempdir().unwrap();
        let d = Draft::open(dir.path().to_path_buf()).unwrap();
        (dir, d)
    }

    #[test]
    fn appends_with_spacing() {
        let (_dir, mut d) = draft();
        d.append("First.").unwrap();
        d.append("Second.").unwrap();
        d.append("\n\nThird.").unwrap();
        assert_eq!(d.text().unwrap(), "First. Second.\n\nThird.");
    }

    #[test]
    fn undo_removes_chunks_newest_first() {
        let (_dir, mut d) = draft();
        d.append("First.").unwrap();
        d.append("Second.").unwrap();
        assert!(d.undo().unwrap());
        assert_eq!(d.text().unwrap(), "First.");
        assert!(d.undo().unwrap());
        assert_eq!(d.text().unwrap(), "");
        assert!(!d.undo().unwrap());
    }

    #[test]
    fn undo_refuses_after_outside_edit() {
        let (_dir, mut d) = draft();
        d.append("Mine.").unwrap();
        fs::write(&d.path, "Edited by hand.").unwrap();
        assert!(!d.undo().unwrap());
        assert_eq!(d.text().unwrap(), "Edited by hand.");
    }

    #[test]
    fn take_archives_and_clears() {
        let (dir, mut d) = draft();
        assert_eq!(d.take().unwrap(), None);
        d.append("Keep me.").unwrap();
        assert_eq!(d.take().unwrap().as_deref(), Some("Keep me."));
        assert_eq!(d.text().unwrap(), "");
        assert!(!d.undo().unwrap());
        let archive = fs::read_to_string(dir.path().join("archive.txt")).unwrap();
        assert!(archive.ends_with("=====\nKeep me."));
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_775), (2026, 11, 18));
    }
}
