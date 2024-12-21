use std::path::PathBuf;

const XDG_DATA_HOME: &str = "XDG_DATA_HOME";
const HOME: &str = "HOME";
const APP_NAME: &str = env!("APP_NAME");

pub fn xdg_data_dir(suffix: &str) -> Option<PathBuf> {
    std::env::var(XDG_DATA_HOME)
        .map(|mut path| {
            path.push('/');
            path.push_str(APP_NAME);
            path.push('/');
            path.push_str(suffix);
            path
        })
        .map(PathBuf::from)
        .ok()
}

pub fn home_data_dir(suffix: &str) -> Option<PathBuf> {
    std::env::var(HOME)
        .map(|mut path| {
            path.push_str("/.local/share/");
            path.push_str(APP_NAME);
            path.push('/');
            path.push_str(suffix);
            path
        })
        .map(PathBuf::from)
        .ok()
}
