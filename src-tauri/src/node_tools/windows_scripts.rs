//! Windows startup text and lossless profile edits, testable on every desktop host.
use crate::config::Result;

const BEGIN: &str = "# >>> fairway node >>>";
const END: &str = "# <<< fairway node <<<";

pub(super) fn powershell_quote(value: &str) -> String {
    if value.is_ascii() {
        format!("'{}'", value.replace('\'', "''"))
    } else {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(value.as_bytes());
        format!("([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded}')))")
    }
}

pub(super) fn cmd_path(value: &str) -> Result<String> {
    if value
        .chars()
        .any(|c| matches!(c, '%' | '!' | '"' | '\r' | '\n'))
    {
        return Err("Windows Node activation requires paths without percent signs, exclamation marks, quotes or line breaks.".into());
    }
    // Rust/Win32 accept mixed separators; CMD can parse a forward slash in an
    // executable or batch path as an option. Always emit native separators.
    Ok(format!("\"{}\"", value.replace('/', "\\")))
}

pub(super) fn powershell_init(fnm: &str, directory: &str, marker: &str) -> String {
    let (fnm, directory, marker) = (
        powershell_quote(fnm),
        powershell_quote(directory),
        powershell_quote(marker),
    );
    format!("if (Test-Path -LiteralPath {marker}) {{\n  $env:FNM_DIR = {directory}\n  & {fnm} env --shell powershell | Out-String | Invoke-Expression\n  $_fairwayNodeDefault = & {fnm} default\n  if ($_fairwayNodeDefault -match '^v?[0-9]+\\.[0-9]+\\.[0-9]+$') {{ & {fnm} use --silent-if-unchanged $_fairwayNodeDefault | Out-Null }}\n  Remove-Variable _fairwayNodeDefault -ErrorAction SilentlyContinue\n}}")
}

pub(super) fn cmd_init(fnm: &str, directory: &str, marker: &str) -> Result<String> {
    let (fnm, marker) = (cmd_path(fnm)?, cmd_path(marker)?);
    cmd_path(directory)?;
    let directory = directory.replace('/', "\\");
    // Read native output from per-session files instead of FOR /F command
    // substitution: its implicit child cmd would rerun the user's AutoRun.
    // Do not CALL returned text (which expands PATH percent signs twice).
    Ok(format!("@echo off\r\nchcp 65001 >nul\r\nif not exist {marker} exit /b\r\nset \"PATH=%PATH:\"=%\"\r\nset \"FNM_DIR={directory}\"\r\nset \"_FAIRWAY_FNM_ENV=%TEMP%\\fairway-fnm-%RANDOM%-%RANDOM%.env\"\r\n{fnm} env --shell cmd > \"%_FAIRWAY_FNM_ENV%\"\r\nif errorlevel 1 goto fairway_fnm_cleanup\r\nfor /f \"usebackq tokens=1,*\" %%A in (\"%_FAIRWAY_FNM_ENV%\") do if /i \"%%A\"==\"SET\" set \"%%B\"\r\n{fnm} default > \"%_FAIRWAY_FNM_ENV%.default\"\r\nfor /f \"usebackq delims=\" %%V in (\"%_FAIRWAY_FNM_ENV%.default\") do {fnm} use --silent-if-unchanged \"%%V\" >nul\r\n:fairway_fnm_cleanup\r\ndel /q \"%_FAIRWAY_FNM_ENV%\" \"%_FAIRWAY_FNM_ENV%.default\" >nul 2>&1\r\nset \"_FAIRWAY_FNM_ENV=\"\r\n"))
}

pub(super) fn profile(bytes: &[u8], init: Option<&str>) -> Result<Vec<u8>> {
    let utf16 = bytes.starts_with(&[0xff, 0xfe]);
    let bom = bytes.starts_with(&[0xef, 0xbb, 0xbf]);
    let previous = if utf16 {
        if !bytes.len().is_multiple_of(2) {
            return Err(
                "PowerShell profile has invalid UTF-16 encoding; it was not changed.".into(),
            );
        }
        let words: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16(&words).map_err(|e| e.to_string())?
    } else {
        std::str::from_utf8(if bom { &bytes[3..] } else { bytes })
            .map_err(|_| "PowerShell profile is not UTF-8 or UTF-16 LE; it was not changed.")?
            .into()
    };
    let mut text = previous;
    while let Some(start) = text.find(BEGIN) {
        let end = start
            + text[start..].find(END).ok_or(
                "The Fairway Node profile block is incomplete; repair it before continuing.",
            )?
            + END.len();
        let end = if text[end..].starts_with("\r\n") {
            end + 2
        } else if text[end..].starts_with('\n') {
            end + 1
        } else {
            end
        };
        text.replace_range(start..end, "");
    }
    if let Some(init) = init {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push_str("\r\n");
        }
        text.push_str(&format!(
            "{BEGIN}\r\n{}\r\n{END}\r\n",
            init.replace('\n', "\r\n")
        ));
    }
    if utf16 {
        Ok([
            vec![0xff, 0xfe],
            text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        ]
        .concat())
    } else {
        // Windows PowerShell 5 reads BOM-less files as ANSI. Add a BOM to new
        // profiles so paths with Unicode work there as well as PowerShell 7.
        Ok([
            if bom || (bytes.is_empty() && init.is_some()) {
                vec![0xef, 0xbb, 0xbf]
            } else {
                vec![]
            },
            text.into_bytes(),
        ]
        .concat())
    }
}

pub(super) fn remove_autorun(previous: &str, hook: &str) -> String {
    if hook.is_empty() {
        return previous.into();
    }
    previous
        .replacen(hook, "", 1)
        .trim_start_matches(" & ")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_edits_preserve_unicode_encoding_and_unrelated_content() {
        let text = "# 日本語\r\nWrite-Host 'mine'\r\n";
        for bytes in [
            text.as_bytes().to_vec(),
            [vec![0xef, 0xbb, 0xbf], text.as_bytes().to_vec()].concat(),
            [
                vec![0xff, 0xfe],
                text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            ]
            .concat(),
        ] {
            let updated = profile(&bytes, Some("Write-Host 'fnm'\n# no switching")).unwrap();
            assert_eq!(
                profile(&updated, Some("Write-Host 'fnm'\n# no switching")).unwrap(),
                updated
            );
            assert_eq!(profile(&updated, None).unwrap(), bytes);
        }
        assert!(profile(BEGIN.as_bytes(), None).is_err());
        assert!(profile(&[0xff, 0xfe, 1], None).is_err());
        assert!(profile(&[0x80], Some("anything")).is_err());
    }
    #[test]
    fn scripts_quote_paths_and_keep_native_fnm_and_exact_default_selection() {
        let path = "C:\\Users\\May's 日本 & tools\\fnm.exe";
        let ps = powershell_init(path, "C:\\node store", "C:\\enabled");
        assert!(ps.is_ascii());
        assert!(ps.contains("FromBase64String"));
        assert_eq!(powershell_quote("May's tools"), "'May''s tools'");
        assert!(ps.contains(" env --shell powershell") && ps.contains(" default"));
        let cmd = cmd_init(path, "C:\\node store", "C:\\enabled").unwrap();
        assert!(cmd.contains("usebackq tokens=1,*"));
        assert!(!cmd.contains(" in ('"));
        assert!(cmd.contains("set \"%%B\""));
        assert!(!cmd.contains("--use-on-cd") && !ps.contains("--use-on-cd"));
        for invalid in ["C:\\%USER%", "C:\\wow!", "C:\\bad\"", "C:\\new\nline"] {
            assert!(cmd_path(invalid).is_err());
        }
    }
    #[test]
    fn cmd_paths_normalize_separators_and_enable_utf8_before_reading_unicode_paths() {
        let script = cmd_init(
            "D:\\build/.runtime-work/fnm-check/日本 & tools/bin/fnm.exe",
            "D:\\build/.runtime-work/fnm-check/日本 & tools/store",
            "D:\\build/.runtime-work/fnm-check/日本 & tools/fnm-enabled",
        )
        .unwrap();
        assert!(script.starts_with("@echo off\r\nchcp 65001 >nul\r\nif not exist "));
        assert!(script.contains(
            "\"D:\\build\\.runtime-work\\fnm-check\\日本 & tools\\bin\\fnm.exe\" env --shell cmd"
        ));
        assert!(script
            .contains("set \"FNM_DIR=D:\\build\\.runtime-work\\fnm-check\\日本 & tools\\store\""));
        assert!(!script.contains("build/") && !script.contains("tools/"));
    }
    #[test]
    fn autorun_removal_preserves_existing_commands_and_later_edits() {
        let hook = " & call \"C:\\Fairway\\node-init.cmd\"";
        assert_eq!(
            remove_autorun(&format!("existing{hook} & later"), hook),
            "existing & later"
        );
        assert_eq!(
            remove_autorun("call \"node.cmd\" & later", "call \"node.cmd\""),
            "later"
        );
        assert_eq!(
            remove_autorun("user replaced startup", hook),
            "user replaced startup"
        );
    }
}
