use std::collections::HashMap;
use std::process::{Command, Stdio};

pub struct UserEnvironment {
    map: HashMap<String, String>
}
impl UserEnvironment {
    pub fn from_user(user: &str) -> Result<UserEnvironment, String> {
        // Executes 'env' as specified user
        let output = Command::new("sudo")
            .arg("-Hiu")
            .arg(user)
            .arg("env")
            // Use NULL character instead of newline because values may also contain newlines
            .arg("-0")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .map_err(|e| e.to_string())?;
        let mut map: HashMap<String, String> = Default::default();
        let mut key = String::new();
        let mut value = String::new();
        let mut passed_equals = false;
        // e.g. "a=1\0b=2\0c=line1\nline2=\0" -> [a: "1", b: "2", c: "line1\nline2="]
        for c in output.stdout {
            if !passed_equals {
                if c == '=' as u8 {
                    passed_equals = true;
                }
                else {
                    key.push(c as char);
                }
            }
            else if c == 0 {
                map.insert(key, value);
                key = String::new();
                value = String::new();
                passed_equals = false;
            }
            else {
                value.push(c as char);
            }
        }
        // We don't insert the final values here because it always ends with a NULL character
        Ok(UserEnvironment { map })
    }
    pub fn get_var(&self, var: &str) -> Option<String> {
        self.map.get(var).map(|s| s.to_string())
    }
}