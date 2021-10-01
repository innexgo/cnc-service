use super::db_types::VerificationChallenge;
use super::utils::current_time_millis;
use tokio_postgres::GenericClient;

impl From<tokio_postgres::row::Row> for VerificationChallenge {
  // select * from user order only, otherwise it will fail
  fn from(row: tokio_postgres::row::Row) -> VerificationChallenge {
    VerificationChallenge {
      verification_challenge_key_hash: row.get("verification_challenge_key_hash"),
      creation_time: row.get("creation_time"),
      creator_user_id: row.get("creator_user_id"),
      to_parent: row.get("to_parent"),
      email: row.get("email"),
    }
  }
}

pub async fn add(
  con: &mut impl GenericClient,
  verification_challenge_key_hash: String,
  email: String,
  creator_user_id: i64,
  to_parent: bool,
) -> Result<VerificationChallenge, tokio_postgres::Error> {
  let creation_time = current_time_millis();

  con.execute(
    "INSERT INTO
     verification_challenge_t(
         verification_challenge_key_hash,
         creation_time,
         creator_user_id,
         to_parent,
         email
     )
     VALUES($1, $2, $3, $4, $5)",
    &[
      &verification_challenge_key_hash,
      &creation_time,
      &creator_user_id,
      &to_parent,
      &email,
    ],
  ).await?;

  Ok(VerificationChallenge {
    verification_challenge_key_hash,
    creation_time,
    creator_user_id,
    to_parent,
    email,
  })
}

pub async fn get_by_verification_challenge_key_hash(
  con: &mut impl GenericClient,
  verification_challenge_key_hash: &str,
) -> Result<Option<VerificationChallenge>, tokio_postgres::Error> {
  let result = con
    .query_opt(
      "SELECT * FROM verification_challenge_t WHERE verification_challenge_key_hash=$1",
      &[&verification_challenge_key_hash],
    ).await?
    .map(|row| row.into());

  Ok(result)
}

pub async fn get_latest_email_time_for_address(
  con: &mut impl GenericClient,
  email: &str,
) -> Result<Option<i64>, tokio_postgres::Error> {
  let time = con
    .query_one(
      "SELECT MAX(creation_time) FROM verification_challenge_t WHERE email=$1",
      &[&email],
    ).await?
    .get(0);

  Ok(time)
}

pub async fn get_latest_time_for_creator(
  con: &mut impl GenericClient,
  creator_user_id: i64,
) -> Result<Option<i64>, tokio_postgres::Error> {
  let time = con
    .query_one(
      "SELECT MAX(creation_time) FROM verification_challenge_t WHERE creator_user_id=$1",
      &[&creator_user_id],
    ).await?
    .get(0);

  Ok(time)
}


pub async fn query(
  con: &mut impl GenericClient,
  props: auth_service_api::request::VerificationChallengeViewProps,
) -> Result<Vec<VerificationChallenge>, tokio_postgres::Error> {
  let results = con
    .query(
      "SELECT vc.* FROM verification_challenge_t vc WHERE 1 = 1
       AND ($1::bigint   IS NULL OR vc.creation_time >= $1)
       AND ($2::bigint   IS NULL OR vc.creation_time <= $2)
       AND ($3::bigint[] IS NULL OR vc.creator_user_id = ANY($3))
       AND ($4::bool     IS NULL OR vc.to_parent = $4)
       AND ($5::text[]   IS NULL OR vc.email = ANY($5))
       ORDER BY vc.verification_challenge_key_hash
      ",
      &[
        &props.min_creation_time,
        &props.max_creation_time,
        &props.creator_user_id,
        &props.to_parent,
        &props.email,
      ],
    )
    .await?
    .into_iter()
    .map(|row| row.into())
    .collect();
  Ok(results)
}
