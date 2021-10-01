use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::convert::TryFrom;
use std::time::{SystemTime, UNIX_EPOCH};
pub fn current_time_millis() -> i64 {
  let since_the_epoch = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("time went backwards");

  since_the_epoch.as_millis() as i64
}

pub fn gen_random_string() -> String {
  // encode 32 bytes of random in base64
  base64_url::encode(&thread_rng().gen::<[u8; 32]>())
}

pub fn hash_str(key: &str) -> String {
  let mut hasher = Sha256::new();
  hasher.update(key);
  let result = hasher.finalize();
  base64_url::encode(&result)
}

pub fn is_secure_password(password: &str) -> bool {
  let len = password.len();

  let numdigits = password.matches(char::is_numeric).count();

  len >= 8 && numdigits > 0
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, argon2::Error> {
  argon2::verify_encoded(password_hash, password.as_bytes())
}

pub fn hash_password(password: &str) -> Result<String, argon2::Error> {
  argon2::hash_encoded(
    // password
    password.as_bytes(),
    // salt
    &thread_rng().gen::<[u8; 32]>(),
    //config
    &argon2::Config::default(),
  )
}


// fun error handling stuff

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SeverityKind {
  Info,
  Warning,
  Error,
  Fatal,
}

impl TryFrom<u8> for SeverityKind {
  type Error = u8;
  fn try_from(val: u8) -> Result<SeverityKind, u8> {
    match val {
      x if x == SeverityKind::Info as u8 => Ok(SeverityKind::Info),
      x if x == SeverityKind::Warning as u8 => Ok(SeverityKind::Warning),
      x if x == SeverityKind::Error as u8 => Ok(SeverityKind::Error),
      x if x == SeverityKind::Fatal as u8 => Ok(SeverityKind::Fatal),
      x => Err(x),
    }
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
  pub msg: String,
  pub source: Option<String>,
  pub severity: SeverityKind,
}

pub fn log(e: Event) {
  println!("{}", serde_json::to_string(&e).unwrap());
}

