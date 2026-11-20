// events.log in the data folder: one line per break started, held or ended, with the reason, so a
// missed or late break can be explained afterwards. Stays on the PC; the oldest half is dropped
// past 256 KB.
use std::io::Write;
use std::path::Path;

const LIMIT: u64 = 256 * 1024;

pub fn write(dir: &Path, text: &str) {
    let path = dir.join("events.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > LIMIT) {
        let _ = std::fs::rename(&path, dir.join("events.old.log"));
    }
    let at = jiff::ZonedDateTime::now().strftime("%Y-%m-%d %H:%M:%S");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{at} {text}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn appends_lines_and_rolls_over_past_the_limit() {
        let dir = std::env::temp_dir().join(format!("bp-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        super::write(&dir, "break started: short");
        super::write(&dir, "break held: full-screen app");
        let text = std::fs::read_to_string(dir.join("events.log")).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.lines().last().unwrap().ends_with(" break held: full-screen app"));
        std::fs::write(dir.join("events.log"), vec![b'x'; super::LIMIT as usize + 1]).unwrap();
        super::write(&dir, "after");
        assert_eq!(std::fs::read_to_string(dir.join("events.log")).unwrap().lines().count(), 1);
        assert!(dir.join("events.old.log").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
