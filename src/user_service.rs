use super::db_types::*;
use super::utils::current_time_millis;
use tokio_postgres::GenericClient;

impl From<tokio_postgres::row::Row> for User {
  // select * from user order only, otherwise it will fail
  fn from(row: tokio_postgres::row::Row) -> User {
    User {
      user_id: row.get("user_id"),
      creation_time: row.get("creation_time"),
    }
  }
}

pub async fn add(
  con: &mut impl GenericClient,
) -> Result<User, tokio_postgres::Error> {
  let creation_time = current_time_millis();
  let user_id = con
    .query_one(
      "INSERT INTO
       user_t(
        creation_time
       )
       VALUES($1)
       RETURNING user_id
      ",
      &[&creation_time],
    )
    .await?
    .get(0);

  // return user
  Ok(User {
    user_id,
    creation_time,
  })
}

pub async fn get_by_user_id(
  con: &mut impl GenericClient,
  user_id: i64,
) -> Result<Option<User>, tokio_postgres::Error> {
  let result = con
    .query_opt("SELECT * FROM user_t WHERE user_id=$1", &[&user_id])
    .await?
    .map(|row| row.into());

  Ok(result)
}

#[allow(unused)]
pub async fn exists_by_user_id(
  con: &mut impl GenericClient,
  user_id: i64,
) -> Result<bool, tokio_postgres::Error> {
  let count: i64 = con
    .query_one("SELECT count(*) FROM user_t WHERE user_id=$1", &[&user_id])
    .await?
    .get(0);
  Ok(count != 0)
}

pub async fn query(
  con: &mut impl GenericClient,
  props: auth_service_api::request::UserViewProps,
) -> Result<Vec<User>, tokio_postgres::Error> {
  let results = con
    .query(
      "SELECT u.* FROM user_t u WHERE 1 = 1
       AND ($1::bigint[] IS NULL OR u.user_id = ANY($1))
       AND ($2::bigint   IS NULL OR u.creation_time >= $2)
       AND ($3::bigint   IS NULL OR u.creation_time <= $3)
       ORDER BY u.user_id
      ",
      &[
        &props.user_id,
        &props.min_creation_time,
        &props.max_creation_time,
      ],
    )
    .await?
    .into_iter()
    .map(|row| row.into())
    .collect();
  Ok(results)
}
