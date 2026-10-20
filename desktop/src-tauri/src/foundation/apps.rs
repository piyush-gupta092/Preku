//! Helpers for showing application names.

/// `SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify` → `Spotify`, `chrome.exe` → `Chrome`.
pub fn friendly_app_name(id: &str) -> Option<String> {
    let name = id.rsplit('!').next().unwrap_or(id);
    let name = name.rsplit(['\\', '/']).next().unwrap_or(name);
    let name = name.strip_suffix(".exe").unwrap_or(name);
    let name = name.rsplit('.').next().unwrap_or(name).trim();
    // Browsers report media sessions with numeric ids; those aren't worth showing.
    if name.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut chars = name.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().chain(chars).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_names() {
        assert_eq!(
            friendly_app_name("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify").as_deref(),
            Some("Spotify")
        );
        assert_eq!(friendly_app_name("chrome.exe").as_deref(), Some("Chrome"));
        assert_eq!(
            friendly_app_name(r"C:\Apps\vlc.exe").as_deref(),
            Some("Vlc")
        );
        assert_eq!(
            friendly_app_name("org.mpris.MediaPlayer2.firefox").as_deref(),
            Some("Firefox")
        );
        assert_eq!(friendly_app_name(""), None);
        assert_eq!(friendly_app_name("1734946747"), None);
    }
}
