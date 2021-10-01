use super::db_types::PasswordReset;
use super::utils::current_time_millis;
use tokio_postgres::GenericClient;

pub async fn add(
  con: &mut impl GenericClient,
  password_reset_key_hash: String,
  creator_user_id: i64,
) -> Result<PasswordReset, tokio_postgres::Error> {
  let creation_time = current_time_millis();
  con
    .execute(
      "
    INSERT INTO password_reset_t(
        password_reset_key_hash,
        creation_time,
        creator_user_id
    ) VALUES ($1, $2, $3)",
      &[&password_reset_key_hash, &creation_time, &creator_user_id],
    )
    .await?;

  Ok(PasswordReset {
    password_reset_key_hash,
    creation_time,
    creator_user_id,
  })
}

pub async fn get_by_password_reset_key_hash(
  con: &mut impl GenericClient,
  password_reset_key_hash: &str,
) -> Result<Option<PasswordReset>, tokio_postgres::Error> {
  let result = con
    .query_opt(
      "SELECT * FROM password_reset_t WHERE password_reset_key_hash=$1",
      &[&password_reset_key_hash],
    )
    .await?
    .map(|row| PasswordReset {
      password_reset_key_hash: row.get("password_reset_key_hash"),
      creation_time: row.get("creation_time"),
      creator_user_id: row.get("creator_user_id"),
    });

  Ok(result)
}
