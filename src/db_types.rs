use auth_service_api::request::ApiKeyKind;

#[derive(Clone, Debug)]
pub struct User {
  pub user_id: i64,
  pub creation_time: i64,
}

#[derive(Clone, Debug)]
pub struct UserData {
  pub user_data_id: i64,
  pub creation_time: i64,
  pub creator_user_id: i64,
  pub name: String,
}

#[derive(Clone, Debug)]
pub struct VerificationChallenge {
  pub verification_challenge_key_hash: String,
  pub creation_time: i64,
  pub creator_user_id: i64,
  pub to_parent: bool,
  pub email: String,
}

#[derive(Clone, Debug)]
pub struct Email {
  pub email_id: i64,
  pub creation_time: i64,
  pub creator_user_id: i64,
  pub verification_challenge_key_hash: String,
}

#[derive(Clone, Debug)]
pub struct ParentPermission {
  pub parent_permission_id: i64,
  pub creation_time: i64,
  pub user_id: i64,
  pub verification_challenge_key_hash: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PasswordReset {
  pub password_reset_key_hash: String,
  pub creation_time: i64,
  pub creator_user_id: i64,
}

#[derive(Clone, Debug)]
pub struct Password {
  pub password_id: i64,
  pub creation_time: i64,
  pub creator_user_id: i64,
  pub password_hash: String,
  pub password_reset_key_hash: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ApiKey {
  pub api_key_id: i64,
  pub creation_time: i64,
  pub creator_user_id: i64,
  pub api_key_hash: String,
  pub api_key_kind: ApiKeyKind,
  pub duration: i64,
}
