//! The yes/no confirmation prompt.

use std::io::{self, BufRead, Write};

/// Writes `prompt` to `output`, reads one line from `input`, and returns whether
/// the answer is `y` or `yes` in any letter case. Any other answer, an empty line,
/// and end of input decline.
///
/// If the line does not end with a newline (for example at end of input),
/// a newline is written so that later output starts on its own line.
pub fn confirm(
    input: &mut impl BufRead,
    output: &mut impl Write,
    prompt: &str,
) -> io::Result<bool> {
    write!(output, "{prompt}")?;
    output.flush()?;
    let mut line = String::new();
    input.read_line(&mut line)?;
    if !line.ends_with('\n') {
        writeln!(output)?;
    }
    let answer = line.trim();
    Ok(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `confirm` with `input` and returns the answer and everything written.
    fn answer(input: &str) -> (bool, String) {
        let mut output = Vec::new();
        let confirmed = confirm(&mut input.as_bytes(), &mut output, "Delete? ").unwrap();
        (confirmed, String::from_utf8(output).unwrap())
    }

    // `y`, `Y`, `yes`, and `YES` confirm.
    #[test]
    fn yes_answers_confirm() {
        for input in ["y\n", "Y\n", "yes\n", "YES\n", "  yes  \n"] {
            assert!(answer(input).0, "{input:?} should confirm");
        }
    }

    // `n`, an empty line, and other text decline.
    #[test]
    fn other_answers_decline() {
        for input in ["n\n", "\n", "no\n", "yess\n", "sure\n"] {
            assert!(!answer(input).0, "{input:?} should decline");
        }
    }

    // End of input declines.
    #[test]
    fn eof_declines() {
        assert!(!answer("").0);
    }

    // The prompt is written, and a newline follows it when the input has none.
    #[test]
    fn prompt_is_written() {
        assert_eq!(answer("y\n").1, "Delete? ");
        assert_eq!(answer("").1, "Delete? \n");
    }
}
