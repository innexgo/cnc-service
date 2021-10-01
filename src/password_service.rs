use super::db_types::*;
use super::utils::current_time_millis;
use tokio_postgres::GenericClient;

impl From<tokio_postgres::row::Row> for Password {
  // select * from password order only, otherwise it will fail
  fn from(row: tokio_postgres::row::Row) -> Password {
    Password {
      password_id: row.get("password_id"),
      creation_time: row.get("creation_time"),
      creator_user_id: row.get("creator_user_id"),
      password_hash: row.get("password_hash"),
      password_reset_key_hash: row.get("password_reset_key_hash"),
    }
  }
}

pub async fn add(
  con: &mut impl GenericClient,
  creator_user_id: i64,
  password_hash: String,
  password_reset_key_hash: Option<String>,
) -> Result<Password, tokio_postgres::Error> {
  let creation_time = current_time_millis();

  let password_id = con
    .query_one(
      "INSERT INTO
       password_t(
         creation_time,
         creator_user_id,
         password_hash,
         password_reset_key_hash
       )
       VALUES ($1, $2, $3, $4)
       RETURNING password_id
      ",
      &[
        &creation_time,
        &creator_user_id,
        &password_hash,
        &password_reset_key_hash,
      ],
    ).await?
    .get(0);

  // return password
  Ok(Password {
    password_id,
    creation_time,
    creator_user_id,
    password_hash,
    password_reset_key_hash,
  })
}

pub async fn get_by_user_id(
  con: &mut impl GenericClient,
  user_id: i64,
) -> Result<Option<Password>, tokio_postgres::Error> {
  let result = con
    .query_opt(
      "SELECT p.* FROM password_t p
       INNER JOIN (SELECT max(password_id) id FROM password_t GROUP BY creator_user_id) maxids ON maxids.id = p.password_id
       WHERE p.creator_user_id = $1
      ",
      &[&user_id],
    ).await?
    .map(|x| x.into());

  Ok(result)
}

#[allow(unused)]
pub async fn get_by_password_id(
  con: &mut impl GenericClient,
  password_id: i64,
) -> Result<Option<Password>, tokio_postgres::Error> {
  let result = con
    .query_opt(
      "SELECT * FROM password_t WHERE password_id=$1",
      &[&password_id],
    ).await?
    .map(|x| x.into());
  Ok(result)
}

pub async fn exists_by_password_reset_key_hash(
  con: &mut impl GenericClient,
  password_reset_key_hash: &str,
) -> Result<bool, tokio_postgres::Error> {
  let count: i64 = con
    .query_one(
      "SELECT count(*) FROM password_t WHERE password_reset_key_hash=$1",
      &[&password_reset_key_hash],
    ).await?
    .get(0);
  Ok(count != 0)
}

pub async fn query(
  con: &mut impl GenericClient,
  props: auth_service_api::request::PasswordViewProps,
) -> Result<Vec<Password>, tokio_postgres::Error> {
  let sql = [

    "SELECT p.* FROM password_t p",
    if props.only_recent {
        " INNER JOIN (SELECT max(password_id) id FROM password_t GROUP BY creator_user_id) maxids ON maxids.id = p.password_id"
    } else {
        ""
    },
    " AND ($1::bigint[] IS NULL OR p.password_id = $1)",
    " AND ($2::bigint   IS NULL OR p.creation_time >= $2)",
    " AND ($3::bigint   IS NULL OR p.creation_time <= $3)",
    " AND ($4::bigint[] IS NULL OR p.creator_user_id = $4)",
    " AND ($5::bool     IS NULL OR p.password_reset_key_hash IS NOT NULL = $5)",
    " ORDER BY p.password_id",
  ]
  .join("");

  let stmnt = con.prepare(&sql).await?;

  let results = con
    .query(
      &stmnt,
      &[
        &props.password_id,
        &props.min_creation_time,
        &props.max_creation_time,
        &props.creator_user_id,
        &props.from_reset,
      ],
    ).await?
    .into_iter()
    .map(|x| x.into())
    .collect();
  Ok(results)
}
