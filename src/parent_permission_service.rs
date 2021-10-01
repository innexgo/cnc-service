use super::db_types::*;
use super::utils::current_time_millis;
use tokio_postgres::GenericClient;

impl From<tokio_postgres::row::Row> for ParentPermission {
  // select * from user order only, otherwise it will fail
  fn from(row: tokio_postgres::row::Row) -> ParentPermission {
    ParentPermission {
      parent_permission_id: row.get("parent_permission_id"),
      creation_time: row.get("creation_time"),
      user_id: row.get("user_id"),
      verification_challenge_key_hash: row.get("verification_challenge_key_hash"),
    }
  }
}

pub async fn add(
  con: &mut impl GenericClient,
  user_id: i64,
  verification_challenge_key_hash: Option<String>,
) -> Result<ParentPermission, tokio_postgres::Error> {
  let creation_time = current_time_millis();

  let parent_permission_id = con
    .query_one(
      "INSERT INTO
       parent_permission_t(
        creation_time,
        user_id,
        verification_challenge_key_hash
       )
       VALUES($1, $2, $3)
       RETURNING parent_permission_id
      ",
      &[&creation_time, &user_id, &verification_challenge_key_hash],
    )
    .await?
    .get(0);

  // return user
  Ok(ParentPermission {
    parent_permission_id,
    user_id,
    creation_time,
    verification_challenge_key_hash,
  })
}

pub async fn get_by_parent_permission_id(
  con: &mut impl GenericClient,
  parent_permission_id: i64,
) -> Result<Option<ParentPermission>, tokio_postgres::Error> {
  let result = con
    .query_opt(
      "SELECT * FROM parent_permission_t WHERE parent_permission_id=$1",
      &[&parent_permission_id],
    )
    .await?
    .map(|row| row.into());

  Ok(result)
}

pub async fn get_by_verification_challenge_key_hash(
  con: &mut impl GenericClient,
  verification_challenge_key_hash: &str,
) -> Result<Option<ParentPermission>, tokio_postgres::Error> {
  let result = con
    .query_opt("SELECT * FROM parent_permission_t WHERE verification_challenge_key_hash=$1", &[&verification_challenge_key_hash])
    .await?
    .map(|row| row.into());

  Ok(result)
}

// gets most recent by user id
pub async fn get_by_user_id(
  con: &mut impl GenericClient,
  user_id: i64,
) -> Result<Option<ParentPermission>, tokio_postgres::Error> {
  let result = con
    .query_opt(
      "SELECT pp.* FROM parent_permission_t pp
       INNER JOIN (SELECT max(parent_permission_id) id FROM parent_permission_t GROUP BY user_id) maxids ON maxids.id = pp.parent_permission_id
       WHERE pp.user_id = $1
      ",
      &[&user_id],
    ).await?
    .map(|x| x.into());

  Ok(result)
}

pub async fn query(
  con: &mut impl GenericClient,
  props: auth_service_api::request::ParentPermissionViewProps,
) -> Result<Vec<ParentPermission>, tokio_postgres::Error> {
  let sql = [
    "SELECT pp.* FROM parent_permission_t pp",
    if props.only_recent {
      " INNER JOIN (SELECT max(parent_permission_id) id FROM parent_permission_t GROUP BY user_id) maxids
        ON maxids.id = e.parent_permission_id"
    } else {
      ""
    },
    " JOIN verification_challenge vc ON vc.verification_challenge_key_hash = e.verification_challenge_key_hash",
    " AND ($1::bigint[] IS NULL OR pp.parent_permission_id = ANY($1))",
    " AND ($2::bigint   IS NULL OR pp.creation_time >= $2)",
    " AND ($3::bigint   IS NULL OR pp.creation_time <= $3)",
    " AND ($4::bigint[] IS NULL OR pp.user_id = ANY($4))",
    " AND ($5::bool     IS NULL OR pp.verification_challenge_key_hash IS NOT NULL = $5)",
    " AND ($6::text[]   IS NULL OR vc.email = ANY($6))",
    " ORDER BY pp.parent_permission_id",
  ]
  .join("\n");

  let stmnt = con.prepare(&sql).await?;

  let results = con
    .query(
      &stmnt,
      &[
        &props.parent_permission_id,
        &props.min_creation_time,
        &props.max_creation_time,
        &props.user_id,
        &props.from_challenge,
        &props.parent_email,
      ],
    )
    .await?
    .into_iter()
    .map(|row| row.into())
    .collect();
  Ok(results)
}
