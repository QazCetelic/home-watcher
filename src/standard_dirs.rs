use crate::user_env::UserEnvironment;

// XDG directories which are usually at $HOME
const XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";
const XDG_CACHE_HOME: &str = "XDG_CACHE_HOME";
const XDG_DATA_HOME: &str = "XDG_DATA_HOME";
const XDG_STATE_HOME: &str = "XDG_STATE_HOME";

pub struct StandardDirectories {
    home: String,
    config_home: String,
    cache_home: String,
    data_home: String,
    state_home: String,
}

impl StandardDirectories {
    pub fn new(user: &str, user_env: &UserEnvironment) -> StandardDirectories {
        let home_dir = user_env.get_var("HOME").unwrap_or_else(|| format!("/home/{user}"));
        StandardDirectories {
            home: home_dir.clone(),
            config_home: user_env.get_var(XDG_CONFIG_HOME).unwrap_or_else(|| format!("{home_dir}/.config")),
            cache_home: user_env.get_var(XDG_CACHE_HOME).unwrap_or_else(|| format!("{home_dir}/.cache")),
            data_home: user_env.get_var(XDG_DATA_HOME).unwrap_or_else(|| format!("{home_dir}/.local/share")),
            state_home: user_env.get_var(XDG_STATE_HOME).unwrap_or_else(|| format!("{home_dir}/.local/state")),
        }
    }
    pub fn home(&self) -> &str { &self.home }
    #[allow(dead_code)]
    pub fn config_home(&self) -> &str { &self.config_home }
    #[allow(dead_code)]
    pub fn cache_home(&self) -> &str { &self.cache_home }
    pub fn data_home(&self) -> &str { &self.data_home }
    #[allow(dead_code)]
    pub fn state_home(&self) -> &str { &self.state_home }
    pub fn all_subdirectories(&self) -> Vec<&str> { vec![&self.config_home, &self.cache_home, &self.data_home, &self.state_home] }
}