use super::db_types::*;
use super::utils::current_time_millis;
use tokio_postgres::GenericClient;

impl From<tokio_postgres::row::Row> for Email {
  // select * from user order only, otherwise it will fail
  fn from(row: tokio_postgres::row::Row) -> Email {
    Email {
      email_id: row.get("email_id"),
      creation_time: row.get("creation_time"),
      creator_user_id: row.get("creator_user_id"),
      verification_challenge_key_hash: row.get("verification_challenge_key_hash"),
    }
  }
}

pub async fn add(
  con: &mut impl GenericClient,
  creator_user_id: i64,
  verification_challenge_key_hash: String,
) -> Result<Email, tokio_postgres::Error> {
  let creation_time = current_time_millis();

  let email_id = con
    .query_one(
      "INSERT INTO
       email_t(
        creation_time,
        creator_user_id,
        verification_challenge_key_hash
       )
       VALUES($1, $2, $3)
       RETURNING email_id
      ",
      &[
        &creation_time,
        &creator_user_id,
        &verification_challenge_key_hash,
      ],
    )
    .await?
    .get(0);

  // return user
  Ok(Email {
    email_id,
    creator_user_id,
    creation_time,
    verification_challenge_key_hash,
  })
}

#[allow(unused)]
pub async fn get_by_email_id(
  con: &mut impl GenericClient,
  email_id: i64,
) -> Result<Option<Email>, tokio_postgres::Error> {
  let result = con
    .query_opt("SELECT * FROM email_t WHERE email_id=$1", &[&email_id])
    .await?
    .map(|row| row.into());

  Ok(result)
}

pub async fn get_by_verification_challenge_key_hash(
  con: &mut impl GenericClient,
  verification_challenge_key_hash: &str,
) -> Result<Option<Email>, tokio_postgres::Error> {
  let result = con
    .query_opt("SELECT * FROM email_t WHERE verification_challenge_key_hash=$1", &[&verification_challenge_key_hash])
    .await?
    .map(|row| row.into());

  Ok(result)
}

// gets most recent email
pub async fn get_by_email(
  con: &mut impl GenericClient,
  email: &str,
) -> Result<Option<Email>, tokio_postgres::Error> {
  let result = con
    .query_opt(
      "SELECT e.* FROM email_t e
       INNER JOIN (SELECT max(email_id) id FROM email_t GROUP BY creator_user_id) maxids ON maxids.id = e.email_id
       INNER JOIN verification_challenge_t vc ON vc.verification_challenge_key_hash = e.verification_challenge_key_hash
       WHERE vc.email = $1
      ",
      &[&email],
    ).await?
    .map(|x| x.into());

  Ok(result)
}

pub async fn get_by_user_id(
  con: &mut impl GenericClient,
  user_id: i64,
) -> Result<Option<Email>, tokio_postgres::Error> {
  let result = con
    .query_opt(
      "SELECT e.* FROM email_t e
       INNER JOIN (SELECT max(email_id) id FROM email_t GROUP BY creator_user_id) maxids ON maxids.id = e.email_id
       WHERE e.creator_user_id = $1
      ",
      &[&user_id],
    ).await?
    .map(|x| x.into());

  Ok(result)
}

pub async fn query(
  con: &mut impl GenericClient,
  props: auth_service_api::request::EmailViewProps,
) -> Result<Vec<Email>, tokio_postgres::Error> {
  let sql = [
    "SELECT e.* FROM email_t e",
    if props.only_recent {
      " INNER JOIN (SELECT max(email_id) id FROM email_t GROUP BY creator_user_id) maxids
        ON maxids.id = e.email_id"
    } else {
      ""
    },
    " JOIN verification_challenge_t vc ON vc.verification_challenge_key_hash = e.verification_challenge_key_hash",
    " AND ($1::bigint[] IS NULL OR e.email_id = ANY($1))",
    " AND ($2::bigint   IS NULL OR e.creation_time >= $2)",
    " AND ($3::bigint   IS NULL OR e.creation_time <= $3)",
    " AND ($4::bigint[] IS NULL OR e.creator_user_id = ANY($4))",
    " AND ($5::text[]   IS NULL OR vc.email = ANY($5))",
    " ORDER BY e.email_id",
  ]
  .join("\n");

  let stmnt = con.prepare(&sql).await?;

  let results = con
    .query(
      &stmnt,
      &[
        &props.email_id,
        &props.min_creation_time,
        &props.max_creation_time,
        &props.creator_user_id,
        &props.email,
      ],
    )
    .await?
    .into_iter()
    .map(|row| row.into())
    .collect();
  Ok(results)
}
